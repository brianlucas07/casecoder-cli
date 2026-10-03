use futures_util::StreamExt;
use serde::Deserialize;
use std::io::{self, Write};

#[derive(Deserialize, Debug, Clone)]
pub struct Modelo {
    pub name: String,
}

#[derive(Deserialize, Debug)]
pub struct RespostaModelos {
    pub models: Vec<Modelo>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: Option<serde_json::Value>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ToolCall {
    pub function: ToolCallFunction,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Message {
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Deserialize, Debug)]
struct StreamChunk {
    #[serde(default)]
    message: Option<Message>,
    #[serde(default)]
    done: bool,
}

pub async fn obter_modelos() -> Option<Vec<Modelo>> {
    let client = reqwest::Client::new();
    let resposta = client
        .get("http://localhost:11434/api/tags")
        .send()
        .await
        .ok()?;

    if resposta.status().is_success() {
        let resposta_json: RespostaModelos = resposta.json().await.ok()?;
        Some(resposta_json.models)
    } else {
        eprintln!("Erro na requisição ao Ollama: {}", resposta.status());
        None
    }
}

pub async fn ia_processar(prompt: &str, modelo: &str, historico: &str, system: &str) -> Option<String> {
    let client = reqwest::Client::new();
    let mut messages = Vec::new();

    if !system.trim().is_empty() {
        messages.push(serde_json::json!({
            "role": "system",
            "content": system
        }));
    }

    if !historico.trim().is_empty() {
        messages.push(serde_json::json!({
            "role": "user",
            "content": format!("Histórico de interações anteriores:\n{}", historico)
        }));
        messages.push(serde_json::json!({
            "role": "assistant",
            "content": "Entendido. Estou pronto para a próxima solicitação."
        }));
    }

    messages.push(serde_json::json!({
        "role": "user",
        "content": prompt
    }));

    let tools_val = crate::memory::obter_tools_value();
    let mut payload = serde_json::json!({
        "model": modelo,
        "messages": messages,
        "stream": true
    });

    if let Some(arr) = tools_val.as_array() {
        if !arr.is_empty() {
            payload["tools"] = tools_val;
        }
    }

    let resposta = match client
        .post("http://localhost:11434/api/chat")
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
    {
        Ok(res) => res,
        Err(e) => {
            eprintln!("Erro ao conectar com Ollama: {e}");
            return None;
        }
    };

    if !resposta.status().is_success() {
        eprintln!("Erro na requisição ao Ollama: {}", resposta.status());
        return None;
    }

    let mut stream = resposta.bytes_stream();
    let mut texto_completo = String::new();
    let mut buffer_linhas = String::new();
    let mut buffer_inicial = String::new();
    let mut modo_streaming_ativo = false;

    while let Some(item) = stream.next().await {
        let bytes = match item {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Erro na conexão de stream: {e}");
                break;
            }
        };

        let pedaco = String::from_utf8_lossy(&bytes);
        buffer_linhas.push_str(&pedaco);

        while let Some(pos) = buffer_linhas.find('\n') {
            let linha = buffer_linhas[..pos].trim().to_string();
            buffer_linhas = buffer_linhas[pos + 1..].to_string();

            if linha.is_empty() {
                continue;
            }

            if let Ok(chunk) = serde_json::from_str::<StreamChunk>(&linha) {
                if let Some(msg) = chunk.message {
                    if let Some(calls) = msg.tool_calls {
                        if let Some(call) = calls.into_iter().next() {
                            let tool_json = serde_json::json!({
                                "name": call.function.name,
                                "arguments": call.function.arguments.unwrap_or(serde_json::json!({}))
                            });
                            return Some(tool_json.to_string());
                        }
                    }

                    let token = msg.content;
                    texto_completo.push_str(&token);

                    if let Some(json_val) = extrair_comando_ferramenta(&texto_completo) {
                        return Some(json_val);
                    }

                    if !modo_streaming_ativo {
                        buffer_inicial.push_str(&token);
                        let trim = buffer_inicial.trim_start();

                        // Se parecer o início de um JSON, silencia o terminal
                        if trim.starts_with('{') || trim.starts_with("```") || trim.contains("\"name\"") {
                            // Aguardando fechar o JSON em silêncio...
                        } else if trim.len() >= 15 || trim.contains('\n') {
                            modo_streaming_ativo = true;
                            print!("{}", buffer_inicial);
                            let _ = io::stdout().flush();
                        }
                    } else {
                        // Se começou a gerar um bloco JSON de ferramenta, silencia imediatamente para não vazar no terminal
                        if !texto_completo.contains('{') {
                            print!("{}", token);
                            let _ = io::stdout().flush();
                        }
                    }
                }

                if chunk.done {
                    break;
                }
            }
        }
    }

    if modo_streaming_ativo {
        println!();
    } else if extrair_comando_ferramenta(&texto_completo).is_none() && !texto_completo.trim().is_empty() {
        println!("{}", texto_completo.trim());
    }

    Some(texto_completo)
}

fn extrair_comando_ferramenta(texto: &str) -> Option<String> {
    if let Some(inicio) = texto.find('{') {
        if let Some(fim) = texto.rfind('}') {
            if fim > inicio {
                let bloco = &texto[inicio..=fim];
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(bloco) {
                    if val.get("name").is_some() {
                        return Some(bloco.to_string());
                    }
                }
            }
        }
    }
    None
}
