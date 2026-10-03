use colored::Colorize;
use dialoguer::Input;
use crate::ollama::Modelo;

pub fn exibir_banner(modelo: &str) {
    let banner = format!(r#"
  ██████╗ █████╗ ███████╗███████╗
 ██╔════╝██╔══██╗██╔════╝██╔════╝
 ██║     ███████║███████╗█████╗
 ██║     ██╔══██║╚════██║██╔══╝
 ╚██████╗██║  ██║███████║███████╗
  ╚═════╝╚═╝  ╚═╝╚══════╝╚══════╝

CASE CODER CLI
Local AI coding assistant
Model: {}
"#, modelo);

    println!("{}", banner.white());
}

pub fn exibir_ajuda() {
    println!("\n{}\n", "Ajuda do CASE CODER CLI".bold().cyan());
    println!("{}", "Comandos disponíveis:".bold());
    println!("  {}  Exibe esta ajuda.", "/help".green());
    println!("  {}  Troca o modelo ativo do Ollama.", "/model".yellow());
    println!("  {}  Encerra a sessão do assistente.", "/exit".red());
    println!();
    println!("{}", "Como usar:".bold());
    println!("  - Digite sua tarefa ou pergunta no prompt principal.");
    println!("  - Use comandos com barra no início para controlar a sessão.");
    println!("  - O histórico da conversa é armazenado localmente para contexto.");
    println!();
    println!("{}", "Exemplos:".italic());
    println!("  Crie uma API REST em Rust com Actix.");
    println!("  Corrija o bug no módulo de autenticação.");
    println!("  Explique essa estrutura de projeto e sugira melhorias.");
    println!();
}

pub fn selecionar_modelo_interativo(modelos: &[Modelo]) -> Option<String> {
    if modelos.is_empty() {
        println!("Nenhum modelo disponível no Ollama.");
        return None;
    }

    println!("\nModelos disponíveis:");
    for (i, modelo) in modelos.iter().enumerate() {
        println!("{}: {}", i + 1, modelo.name);
    }

    let escolha: usize = Input::new()
        .with_prompt("Escolha o modelo desejado (digite o número correspondente)")
        .interact_text()
        .ok()?;

    if escolha > 0 && escolha <= modelos.len() {
        Some(modelos[escolha - 1].name.clone())
    } else {
        println!("Opção inválida.");
        None
    }
}
