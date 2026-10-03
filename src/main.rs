mod config;
mod editor;
mod memory;
mod ollama;
mod ui;

use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Deserialize, Serialize, Clone)]
struct RespostaJson {
    name: String,
    path: Option<String>,
    #[serde(default)]
    arguments: Option<serde_json::Value>,
}

impl RespostaJson {
    fn obter_path(&self) -> Option<String> {
        if let Some(p) = &self.path {
            return Some(p.clone());
        }
        if let Some(args) = &self.arguments {
            match args {
                serde_json::Value::Object(map) => {
                    if let Some(p) = map.get("path").and_then(|v| v.as_str()) {
                        return Some(p.to_string());
                    }
                }
                serde_json::Value::String(s) => {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(s) {
                        if let Some(p) = parsed.get("path").and_then(|v| v.as_str()) {
                            return Some(p.to_string());
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }
}

#[tokio::main]
async fn main() {
    memory::inicializar();

    let mut config = config::carregar_config();

    if config.model.trim().is_empty() || config.model == "Modelo não definido" {
        ui::exibir_banner("Modelo não definido");
        println!("Nenhum modelo configurado. Buscando modelos instalados no Ollama...");

        if let Some(modelos) = ollama::obter_modelos().await {
            if let Some(modelo_escolhido) = ui::selecionar_modelo_interativo(&modelos) {
                config.model = modelo_escolhido.clone();
                let _ = config::salvar_config(&config);
            }
        }
    }

    let modelo_ativo = if config.model.trim().is_empty() {
        "Modelo não definido"
    } else {
        &config.model
    };

    ui::exibir_banner(modelo_ativo);

    let mut rl = match editor::criar_editor() {
        Ok(ed) => ed,
        Err(e) => {
            eprintln!("Erro ao inicializar terminal interativo: {e}");
            return;
        }
    };

    loop {

        let mut prompt = match rl.readline("> ") {
            Ok(linha) => linha,
            Err(rustyline::error::ReadlineError::Interrupted)
            | Err(rustyline::error::ReadlineError::Eof) => break,
            Err(erro) => {
                eprintln!("Erro ao ler entrada: {erro}");
                break;
            }
        };


        let prompt_trimmed = prompt.trim();

        if prompt_trimmed.eq_ignore_ascii_case("/exit") {
            break;
        } else if prompt_trimmed.eq_ignore_ascii_case("/model") {
            if let Some(modelos) = ollama::obter_modelos().await {
                if let Some(novo_modelo) = ui::selecionar_modelo_interativo(&modelos) {
                    config.model = novo_modelo.clone();
                    let _ = config::salvar_config(&config);
                    println!("Modelo alterado para: {}", novo_modelo.green());
                }
            }
            continue;
        } else if prompt_trimmed.eq_ignore_ascii_case("/help") {
            ui::exibir_ajuda();
            continue;
        }

        if prompt_trimmed.is_empty() {
            continue;
        }

        let prompt_usuario = prompt.clone();
        let historico = memory::ler_memoria();
        let system_prompt = memory::prompt_sistema();
        println!("{}", "Processando...".dimmed());

        if let Some(mut resposta_ia) = ollama::ia_processar(&prompt, &config.model, &historico, &system_prompt).await {
            let mut tentativas = 0;
            let mut diretorios_listados: Vec<String> = Vec::new();
            const MAX_TENTATIVAS: usize = 5;

            // Loop de execução de ferramentas: continua enquanto a IA solicitar comandos JSON
            while let Some(resposta_json) = parse_resposta(&resposta_ia) {
                tentativas += 1;
                if tentativas >= MAX_TENTATIVAS {
                    println!("{}", "Aviso: Limite de iterações atingido.".yellow());
                    break;
                }

                // Executa a ferramenta solicitada e gera a observação para a IA
                let observacao = match resposta_json.name.as_str() {
                    "list_directory" => {
                        let path_opt = resposta_json.obter_path();
                        let raw_alvo = path_opt.as_deref().unwrap_or(".");
                        let alvo = if raw_alvo == "/path/to/directory"
                            || raw_alvo == "path/to/directory"
                            || raw_alvo == "<caminho_do_diretorio>"
                            || raw_alvo.trim().is_empty()
                        {
                            "."
                        } else {
                            raw_alvo
                        };

                        if diretorios_listados.contains(&alvo.to_string()) {
                            println!("{}", format!("Aviso: diretório '{}' já foi listado.", alvo).yellow());
                            let aviso = format!("O diretório '{}' já foi listado. Analise a árvore já recebida ou leia os arquivos com 'read_file'.", alvo);
                            formatar_observacao("list_directory", &aviso, &prompt_usuario)
                        } else {
                            diretorios_listados.push(alvo.to_string());
                            println!("{}", format!("Listando diretório: {}", alvo).cyan());
                            match gerar_arvore_diretorio(std::path::Path::new(alvo), 3) {
                                Ok(arvore) => formatar_observacao("list_directory", &arvore, &prompt_usuario),
                                Err(e) => {
                                    let erro_msg = format!("Erro ao listar diretório '{}': {}", alvo, e);
                                    println!("{}", erro_msg.red());
                                    formatar_observacao("list_directory", &erro_msg, &prompt_usuario)
                                }
                            }
                        }
                    }

                    "read_file" => {
                        if let Some(caminho) = resposta_json.obter_path() {
                            println!("{}", format!("Lendo: {}", caminho).cyan());
                            match fs::read_to_string(&caminho) {
                                Ok(conteudo) => formatar_observacao("read_file", &conteudo, &prompt_usuario),
                                Err(e) => {
                                    let erro_msg = format!("Erro ao ler arquivo '{}': {}", caminho, e);
                                    println!("{}", erro_msg.red());
                                    formatar_observacao("read_file", &erro_msg, &prompt_usuario)
                                }
                            }
                        } else {
                            println!("{}", "Aviso: 'read_file' chamado sem caminho ('path').".red());
                            break;
                        }
                    }

                    "list_tools" => {
                        formatar_observacao("list_tools", &memory::obter_tools_json(), &prompt_usuario)
                    }

                    outro => {
                        println!("{}", format!("Ação não suportada: {}", outro).yellow());
                        break;
                    }
                };

                // Alimenta a resposta da ferramenta de volta no prompt e consulta a IA novamente
                prompt.push_str(&observacao);
                resposta_ia = ollama::ia_processar(&prompt, &config.model, &historico, &system_prompt)
                    .await
                    .unwrap_or_default();
            }

            memory::salvar_interacao(&prompt_usuario, &resposta_ia);
        }
    }
}
fn parse_resposta(texto: &str) -> Option<RespostaJson> {
    // 1. Tenta extrair de blocos com ```json ... ``` ou ``` ... ```
    if let Some(pos) = texto.find("```") {
        let resto = &texto[pos + 3..];
        let conteudo = if let Some(depois) = resto.strip_prefix("json") {
            depois
        } else {
            resto
        };
        if let Some(fim) = conteudo.find("```") {
            let bloco = conteudo[..fim].trim();
            if let Ok(res) = serde_json::from_str::<RespostaJson>(bloco) {
                return Some(res);
            }
        }
    }

    // 2. Tenta extrair entre o primeiro '{' e o último '}'
    if let Some(inicio) = texto.find('{') {
        if let Some(fim) = texto.rfind('}') {
            if fim >= inicio {
                let bloco = &texto[inicio..=fim];
                if let Ok(res) = serde_json::from_str::<RespostaJson>(bloco) {
                    return Some(res);
                }
            }
        }
    }

    // 3. Fallback: string inteira
    serde_json::from_str(texto.trim()).ok()
}

fn formatar_observacao(ferramenta: &str, resultado: &str, pergunta_original: &str) -> String {
    format!(
        "\n\n[Resultado da ferramenta '{}']:\n{}\n\nLembre-se da solicitação original do usuário: \"{}\". Baseado nas informações acima, responda à pergunta do usuário com precisão e clareza.",
        ferramenta, resultado, pergunta_original
    )
}

fn gerar_arvore_diretorio(raiz: &std::path::Path, max_depth: usize) -> Result<String, std::io::Error> {
    if !raiz.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Diretório '{}' não encontrado.", raiz.display()),
        ));
    }
    if !raiz.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("'{}' não é um diretório.", raiz.display()),
        ));
    }

    let mut resultado = String::new();
    let nome_raiz = raiz
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| raiz.display().to_string());
    resultado.push_str(&format!("{}/\n", nome_raiz));
    construir_arvore(raiz, "", 0, max_depth, &mut resultado)?;
    Ok(resultado)
}

fn construir_arvore(
    dir: &std::path::Path,
    prefixo: &str,
    nivel_atual: usize,
    max_depth: usize,
    saida: &mut String,
) -> Result<(), std::io::Error> {
    if nivel_atual >= max_depth {
        return Ok(());
    }

    let mut entradas: Vec<_> = match fs::read_dir(dir) {
        Ok(read) => read.filter_map(|e| e.ok()).collect(),
        Err(_) => return Ok(()),
    };

    // Filtra ocultos e pastas pesadas
    entradas.retain(|e| {
        let nome = e.file_name().to_string_lossy().to_string();
        !nome.starts_with('.') && nome != "target" && nome != "node_modules" && nome != "__pycache__"
    });

    // Pastas primeiro, depois arquivos alfabeticamente
    entradas.sort_by_key(|e| {
        let is_dir = e.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
        (!is_dir, e.file_name())
    });

    let total = entradas.len();
    for (i, entrada) in entradas.into_iter().enumerate() {
        let eh_ultimo = i + 1 == total;
        let conector = if eh_ultimo { "└── " } else { "├── " };
        let novo_prefixo = if eh_ultimo { "    " } else { "│   " };

        let nome = entrada.file_name().to_string_lossy().to_string();
        let eh_dir = entrada.file_type().map(|ft| ft.is_dir()).unwrap_or(false);

        if eh_dir {
            saida.push_str(&format!("{}{}{}/\n", prefixo, conector, nome));
            let _ = construir_arvore(
                &entrada.path(),
                &format!("{}{}", prefixo, novo_prefixo),
                nivel_atual + 1,
                max_depth,
                saida,
            );
        } else {
            saida.push_str(&format!("{}{}{}\n", prefixo, conector, nome));
        }
    }

    Ok(())
}