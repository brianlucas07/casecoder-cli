using System;
using System.Collections.Generic;
using System.IO;
using System.Net.Http;
using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Threading.Tasks;
using System.Diagnostics;

public class ChatMessage
{
    [JsonPropertyName("role")]
    public string Role { get; set; } = "";

    [JsonPropertyName("content")]
    public string Content { get; set; } = "";
}

class Ollama
{
    private static readonly HttpClient client = new HttpClient
    {
        BaseAddress = new Uri("http://localhost:11434"),
        Timeout = TimeSpan.FromMinutes(10)
    };

    private static readonly object[] Tools = new object[]
    {
        new
        {
            type = "function",
            function = new
            {
                name = "list_directory",
                description = "Lista todos os arquivos e pastas do diretório de trabalho atual."
            }
        },
        new
        {
            type = "function",
            function = new
            {
                name = "read_file",
                description = "Lê o conteúdo completo de um arquivo no disco.",
                parameters = new
                {
                    type = "object",
                    properties = new
                    {
                        path = new
                        {
                            type = "string",
                            description = "Caminho relativo ou absoluto do arquivo a ser lido (ex: Program.cs, README.md)"
                        }
                    },
                    required = new[] { "path" }
                }
            }
        }
    };

    public static async Task<JsonElement> Chat(List<ChatMessage> messages)
    {
        var payload = new
        {
            model = "qwen2.5-coder:3b",
            messages = messages,
            stream = false,
            tools = Tools
        };

        string jsonPayload = JsonSerializer.Serialize(payload);
        var content = new StringContent(jsonPayload, Encoding.UTF8, "application/json");

        HttpResponseMessage response;
        try
        {
            response = await client.PostAsync("/api/chat", content);
        }
        catch (TaskCanceledException)
        {
            throw new Exception("O Ollama demorou muito para responder e a requisição expirou.");
        }

        if (!response.IsSuccessStatusCode)
        {
            string err = await response.Content.ReadAsStringAsync();
            throw new Exception($"Erro na API do Ollama ({response.StatusCode}): {err}");
        }

        string responseJson = await response.Content.ReadAsStringAsync();
        using var doc = JsonDocument.Parse(responseJson);

        return doc.RootElement.GetProperty("message").Clone();
    }
}

class Program
{
    private const string ArquivoLog = "context.txt";

    static async Task Main()
    {
        // Limpa o log anterior ao iniciar
        await File.WriteAllTextAsync(ArquivoLog, "", Encoding.UTF8);

        Console.ForegroundColor = ConsoleColor.Cyan;
        Console.WriteLine("CaseCoder CLI inicializado (com suporte nativo a ferramentas via Ollama).");
        Console.WriteLine("Digite sua mensagem ou /exit para sair.");
        Console.ResetColor();

        var historico = new List<ChatMessage>
        {
            new ChatMessage
            {
                Role = "system",
                Content = "Você é o CaseCoder, um assistente técnico em uma sessão interativa pelo terminal. Responda sempre em português. " +
                          "Use as ferramentas disponíveis sempre que precisar inspecionar arquivos ou diretórios do usuário antes de responder."
            }
        };

        while (true)
        {
            Console.ForegroundColor = ConsoleColor.DarkBlue;
            Console.WriteLine(new string('\u2500', Math.Max(Console.WindowWidth, 20)));

            Console.ForegroundColor = ConsoleColor.Green;
            Console.Write("> ");

            Console.ForegroundColor = ConsoleColor.White;
            string? prompt = Console.ReadLine();

            if (string.IsNullOrWhiteSpace(prompt)) continue;
            if (prompt.Trim().Equals("/exit", StringComparison.OrdinalIgnoreCase)) break;

            Console.ForegroundColor = ConsoleColor.DarkBlue;
            Console.WriteLine(new string('\u2500', Math.Max(Console.WindowWidth, 20)));

            int linhaStatus = Console.CursorTop;
            historico.Add(new ChatMessage { Role = "user", Content = prompt });

            int maxToolIterations = 4;
            bool respondeu = false;

            while (maxToolIterations-- > 0)
            {
                ExibirStatus(linhaStatus, "Pensando...");

                JsonElement messageElement;
                try
                {
                    messageElement = await Ollama.Chat(historico);
                }
                catch (Exception ex)
                {
                    LimparLinha(linhaStatus);
                    Console.ForegroundColor = ConsoleColor.Red;
                    Console.WriteLine($"Erro: {ex.Message}");
                    Console.ResetColor();
                    respondeu = true;
                    break;
                }

                if (DetectarChamadaFerramenta(messageElement, out string toolName, out string toolArgs))
                {
                    ExibirStatus(linhaStatus, $"Executando ferramenta: {toolName} {toolArgs}...");
                    string toolResult = ExecutarFerramenta(toolName, toolArgs);

                    string assistantContent = messageElement.TryGetProperty("content", out var c) ? c.GetString() ?? "" : "";
                    if (string.IsNullOrWhiteSpace(assistantContent))
                    {
                        assistantContent = $"{{\"name\": \"{toolName}\"}}";
                    }

                    historico.Add(new ChatMessage
                    {
                        Role = "assistant",
                        Content = assistantContent
                    });

                    historico.Add(new ChatMessage
                    {
                        Role = "tool",
                        Content = toolResult
                    });
                }
                else
                {
                    string respostaFinal = messageElement.TryGetProperty("content", out var c) ? c.GetString() ?? "" : "";
                    historico.Add(new ChatMessage { Role = "assistant", Content = respostaFinal });

                    LimparLinha(linhaStatus);
                    Console.ForegroundColor = ConsoleColor.White;
                    Console.WriteLine(respostaFinal);
                    Console.ResetColor();

                    respondeu = true;
                    break;
                }
            }

            if (!respondeu)
            {
                LimparLinha(linhaStatus);
                Console.ForegroundColor = ConsoleColor.Yellow;
                Console.WriteLine("Limite de chamadas de ferramentas atingido.");
                Console.ResetColor();
            }

            await SalvarHistoricoEmArquivo(historico);
        }
    }

    private static bool DetectarChamadaFerramenta(JsonElement messageElement, out string toolName, out string toolArgs)
    {
        toolName = string.Empty;
        toolArgs = string.Empty;

        // 1. Verifica tool_calls nativo da API
        if (messageElement.TryGetProperty("tool_calls", out var toolCalls) &&
            toolCalls.ValueKind == JsonValueKind.Array &&
            toolCalls.GetArrayLength() > 0)
        {
            var firstTool = toolCalls[0];
            if (firstTool.TryGetProperty("function", out var func))
            {
                if (func.TryGetProperty("name", out var nameProp))
                {
                    toolName = nameProp.GetString() ?? "";
                }

                if (func.TryGetProperty("arguments", out var argsProp))
                {
                    if (argsProp.ValueKind == JsonValueKind.Object && argsProp.TryGetProperty("path", out var pathProp))
                    {
                        toolArgs = pathProp.GetString() ?? "";
                    }
                    else
                    {
                        toolArgs = argsProp.ToString();
                    }
                }

                if (!string.IsNullOrEmpty(toolName)) return true;
            }
        }

        // 2. Verifica se o content é um JSON de chamada de função (ex: {"name": "list_directory", ...})
        string content = messageElement.TryGetProperty("content", out var cProp) ? cProp.GetString() ?? "" : "";
        string trimmed = content.Trim();

        if (trimmed.StartsWith("{") && trimmed.EndsWith("}"))
        {
            try
            {
                using var doc = JsonDocument.Parse(trimmed);
                if (doc.RootElement.TryGetProperty("name", out var nameEl))
                {
                    toolName = nameEl.GetString() ?? "";
                    if (doc.RootElement.TryGetProperty("arguments", out var argsEl))
                    {
                        if (argsEl.ValueKind == JsonValueKind.Object && argsEl.TryGetProperty("path", out var pEl))
                        {
                            toolArgs = pEl.GetString() ?? "";
                        }
                        else
                        {
                            toolArgs = argsEl.ToString();
                        }
                    }
                    if (!string.IsNullOrEmpty(toolName)) return true;
                }
            }
            catch
            {
                // Ignora erro de JSON e continua para fallback
            }
        }

        // 3. Fallback para texto plano
        if (trimmed.Equals("list_directory", StringComparison.OrdinalIgnoreCase) ||
            trimmed.Contains("\"name\": \"list_directory\""))
        {
            toolName = "list_directory";
            return true;
        }

        if (trimmed.StartsWith("read_file ", StringComparison.OrdinalIgnoreCase))
        {
            toolName = "read_file";
            toolArgs = trimmed.Substring("read_file ".Length).Trim();
            return true;
        }

        return false;
    }

    private static string ExecutarFerramenta(string nome, string argumento)
    {
        try
        {
            if (nome == "list_directory")
            {
                bool isWindows = RuntimeInformation.IsOSPlatform(OSPlatform.Windows);
                string cmd = isWindows ? "dir" : "ls -la";
                return ExecutarComandoShell(cmd, isWindows);
            }

            if (nome == "read_file")
            {
                if (string.IsNullOrWhiteSpace(argumento))
                {
                    return "Erro: Caminho do arquivo não fornecido.";
                }

                if (!File.Exists(argumento))
                {
                    return $"Erro: Arquivo '{argumento}' não encontrado.";
                }
                return File.ReadAllText(argumento, Encoding.UTF8);
            }

            return $"Ferramenta desconhecida: {nome}";
        }
        catch (Exception ex)
        {
            return $"Erro ao executar {nome}: {ex.Message}";
        }
    }

    private static string ExecutarComandoShell(string comando, bool isWindows)
    {
        var processInfo = new ProcessStartInfo
        {
            FileName = isWindows ? "cmd.exe" : "/bin/bash",
            Arguments = isWindows ? $"/c {comando}" : $"-c \"{comando}\"",
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            UseShellExecute = false,
            CreateNoWindow = true
        };

        using var process = new Process { StartInfo = processInfo };
        process.Start();

        string output = process.StandardOutput.ReadToEnd();
        string error = process.StandardError.ReadToEnd();
        process.WaitForExit();

        if (!string.IsNullOrWhiteSpace(error))
        {
            return $"{output}\n[Avisos/Erros]:\n{error}";
        }

        return output;
    }

    private static void ExibirStatus(int linha, string mensagem)
    {
        try
        {
            LimparLinha(linha);
            Console.ForegroundColor = ConsoleColor.DarkGray;
            Console.Write(mensagem);
            Console.ResetColor();
        }
        catch
        {
            // Caso o terminal não suporte cursor
        }
    }

    private static void LimparLinha(int linha)
    {
        try
        {
            Console.SetCursorPosition(0, linha);
            Console.Write(new string(' ', Console.WindowWidth));
            Console.SetCursorPosition(0, linha);
        }
        catch
        {
            // Ignora se o console não suportar cursor
        }
    }

    private static async Task SalvarHistoricoEmArquivo(List<ChatMessage> historico)
    {
        var sb = new StringBuilder();
        foreach (var msg in historico)
        {
            if (msg.Role == "system") continue;
            sb.AppendLine($"[{msg.Role.ToUpper()}]: {msg.Content}\n");
        }
        await File.WriteAllTextAsync(ArquivoLog, sb.ToString(), Encoding.UTF8);
    }
}

