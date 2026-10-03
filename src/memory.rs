use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

fn memory_file_path() -> PathBuf {
    crate::config::app_dir().join("memory.txt")
}

fn tools_file_path() -> PathBuf {
    crate::config::app_dir().join("tools.json")
}

pub const DEFAULT_TOOLS_JSON: &str = r#"[
  {
    "type": "function",
    "function": {
      "name": "list_directory",
      "description": "List files and subdirectories as a tree structure. Use '.' for the current project root. Automatically reveals subdirectories while ignoring build folders like target/.",
      "parameters": {
        "type": "object",
        "properties": {
          "path": {
            "type": "string",
            "description": "Path to the directory to inspect. Use '.' for the project root."
          }
        }
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "read_file",
      "description": "Read the content of a file. Supports files in subdirectories using relative paths with slashes (e.g. 'src/main.rs').",
      "parameters": {
        "type": "object",
        "properties": {
          "path": {
            "type": "string",
            "description": "Path of the file to read (e.g. 'src/main.rs' or 'Cargo.toml')."
          }
        },
        "required": ["path"]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "write_file",
      "description": "Write content to a file.",
      "parameters": {
        "type": "object",
        "properties": {
          "path": {
            "type": "string",
            "description": "Path of the file to write."
          },
          "content": {
            "type": "string",
            "description": "Content to write to the file."
          }
        },
        "required": ["path", "content"]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "create_directory",
      "description": "Create a new directory.",
      "parameters": {
        "type": "object",
        "properties": {
          "path": {
            "type": "string",
            "description": "Path of the directory to create."
          }
        },
        "required": ["path"]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "delete_file",
      "description": "Delete a file.",
      "parameters": {
        "type": "object",
        "properties": {
          "path": {
            "type": "string",
            "description": "Path of the file to delete."
          }
        },
        "required": ["path"]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "delete_directory",
      "description": "Delete a directory.",
      "parameters": {
        "type": "object",
        "properties": {
          "path": {
            "type": "string",
            "description": "Path of the directory to delete."
          }
        },
        "required": ["path"]
      }
    }
  }
]"#;

pub fn obter_tools_json() -> String {
    let path = tools_file_path();
    if !path.exists() {
        let _ = fs::write(&path, DEFAULT_TOOLS_JSON);
        return DEFAULT_TOOLS_JSON.to_string();
    }
    fs::read_to_string(&path).unwrap_or_else(|_| DEFAULT_TOOLS_JSON.to_string())
}

pub fn obter_tools_value() -> serde_json::Value {
    let json_str = obter_tools_json();
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_str) {
        if let Some(array) = val.get("tools") {
            if array.is_array() {
                let converted: Vec<serde_json::Value> = array
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|item| {
                        serde_json::json!({
                            "type": "function",
                            "function": {
                                "name": item.get("name").and_then(|v| v.as_str()).unwrap_or_default(),
                                "description": item.get("description").and_then(|v| v.as_str()).unwrap_or_default(),
                                "parameters": {
                                    "type": "object",
                                    "properties": {
                                        "path": { "type": "string", "description": "Caminho do arquivo ou pasta" }
                                    }
                                }
                            }
                        })
                    })
                    .collect();
                return serde_json::Value::Array(converted);
            }
        }
        return val;
    }
    serde_json::from_str(DEFAULT_TOOLS_JSON).unwrap_or(serde_json::json!([]))
}

pub fn prompt_sistema() -> String {
    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| ".".to_string());

    format!(
        "Você é um assistente técnico local inteligente. \
Você está executando no diretório de trabalho: '{}'. \
O diretório atual do projeto é representado por '.' (ponto). \
Quando o usuário pedir para listar, mapear ou explorar o projeto, use '.' como path para a ferramenta 'list_directory' para obter a árvore de arquivos e subpastas. NUNCA use placeholders fictícios como '/path/to/directory'. \
Para ler arquivos em subpastas, use sempre o caminho relativo com barras (ex: 'src/main.rs' ou 'src/config.rs'). \
Você tem acesso a ferramentas de sistema operacional para inspecionar e manipular o ambiente. \
Quando precisar invocar uma ferramenta, invoque-a IMEDIATAMENTE e diretamente, SEM anunciar previamente ou enviar mensagens conversacionais antes da ferramenta. \
NUNCA invente nem suponha o conteúdo de arquivos sem lê-los primeiro. \
Para saudações e conversas normais sem ferramentas, responda diretamente em português amigável.",
        cwd
    )
}

pub fn inicializar() {
    let _ = obter_tools_json();
    let _ = fs::write(memory_file_path(), "");
}

pub fn ler_memoria() -> String {
    fs::read_to_string(memory_file_path()).unwrap_or_default()
}

pub fn salvar_interacao(usuario: &str, ia: &str) {
    if let Ok(mut arquivo) = OpenOptions::new().append(true).create(true).open(memory_file_path()) {
        let _ = writeln!(arquivo, "User: {}", usuario);
        let _ = writeln!(arquivo, "IA: {}", ia);
    }
}
