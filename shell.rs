use std::io;
use std::io::Write;
use std::io::prelude::*;
use std::process::Command;
use std::process::exit;
use std::fs::File;
use std::env;

use whoami;

#[link(name = "c")]
unsafe extern "C" {
    fn geteuid() -> u32;
}

struct Variable {
    name: String,
    value: String
}

struct Alias {
    name: String,
    value: String
}

struct State {
    prompt: String,
    aliases: Vec<Alias>,
    variables: Vec<Variable>
}

impl State {
    fn get_var(&self, name: &str) -> &str {
        for var in &self.variables {
            if var.name == name {
                return var.value.as_str();
            }
        }
        return "ERROR";
    }
}

fn builtin_exit(_args: &Vec<String>) {
    exit(0);
}

fn builtin_help(_args: &Vec<String>) {
    println!("help message");
}

fn builtin_cd(args: &Vec<String>) {
    if args.len() < 2 {
        println!("cd: needs atleast 1 arguments");
        return;
    }
    let path_str: &str = &args[1].clone();
    let _ = env::set_current_dir(path_str);
}

fn builtin_alias(args: &Vec<String>, state: &mut State) {
    if args.len() < 3 {
        println!("alias: needs atleast 2 arguments");
        return;
    }
    state.aliases.push({ Alias {
        name: args[1].clone(),
        value: args[2].clone()} });
}

fn builtin_prompt(args: &Vec<String>, state: &mut State) {
    if args.len() < 2 {
        println!("ps1: needs atleast 1 arguments");
        return;
    }
    state.prompt = args[1].clone();
}

fn builtin_set(args: &Vec<String>, state: &mut State) {
    if args.len() < 3 {
        println!("set: needs atleast 2 arguments");
    }
    state.variables.push({ Variable {
        name: args[1].clone(),
        value: args[2].clone()} });
}

fn prompt_to_string(state: &State) -> String {
    let username = whoami::username().expect("failed to get username");
    let hostname = whoami::hostname().expect("failed to get hostname");
    let working_dir = env::current_dir().expect("failed to get working directory")
        .as_os_str().to_str().expect("failed to convert to string")
        .to_string();
    let working_dir_last = env::current_dir().expect("failed to get working directory")
        .components().last().expect("failed to get last path")
        .as_os_str().to_str().expect("failed to convert to string")
        .to_string();
    let perm_symbol: &str;

    unsafe {
        if geteuid() == 0 {
            perm_symbol = "#";
        } else {
            perm_symbol = "$";
        }
    }

    let string_vec: Vec<char> = state.prompt.chars().collect();

    let mut prompt = String::new();
    let mut c: char;
    let mut i: usize = 0;

    while i < string_vec.len() {
        c = string_vec[i].clone();
        if c == '\\' {
            i += 1;  // skip ahead to the escaped character
            match string_vec[i].clone() {
                'u' => prompt.push_str(username.as_str()),
                'h' => prompt.push_str(hostname.as_str()),
                'w' => prompt.push_str(working_dir.as_str()),
                'W' => prompt.push_str(working_dir_last.as_str()),
                '$' => prompt.push_str(perm_symbol),
                _ => {}
            }
        } else {
            prompt.push(c.clone());
        }
        i += 1;
    }

    return prompt;
}

fn read_input() -> String {
    let mut buffer = String::new();
    let _ = io::stdin().read_line(&mut buffer);

    if buffer.ends_with('\n') {
        buffer.pop();
        if buffer.ends_with('\r') {
            buffer.pop();
        }
    }

    return buffer;
}

fn parse_input(input: &String, state: &mut State) -> Vec<String> {
    if input.len() == 0 {
        return Vec::new();
    }

    let mut string: Vec<u8> = Vec::new();
    let mut args: Vec<String> = Vec::new();
    let mut buffer: Vec<u8> = Vec::new();
    let mut c: u8;
    let mut i: usize = 0;
    let mut quote: bool = false;

    let _ = string.write(input.as_bytes());

    while i < string.len() {
        c = string[i].clone();
        if i == string.len() - 1 {
            if c != b'"' {
                buffer.push(c.clone());
            }
            args.push(String::from_utf8(buffer.clone()).unwrap());
            buffer.clear(); 
            break;
        }
        if c == b'"' {
            if !quote {
                quote = true;
            } else {
                quote = false;
                //args.push(String::from_utf8(buffer.clone()).unwrap());
                //buffer.clear();
            }
        } else if c == b' ' {
            if quote { 
                buffer.push(c.clone());
                i += 1;
                continue; 
            }
            args.push(String::from_utf8(buffer.clone()).unwrap());
            buffer.clear();
        } else {
            buffer.push(c.clone());
        }
        i += 1;
    }

    // deal with variables
    for i in 0..args.len() {
        let first = args[i].chars().next().unwrap();

        let mut x = args[i].chars();
        x.next();
        let var = x.as_str();

        if first == '$' {
            args[i] = state.get_var(var).to_string();
        }
    }

    return args;
}

fn execute_command(args: &Vec<String>, state: &mut State) {
    if args[0].clone() == "" {
        return;
    }

    let mut command: &str = &args[0].clone();
    let mut args_1: Vec<String> = args.clone();
    args_1.remove(0);
    
    for alias in state.aliases.iter() {
        if command == alias.name.clone() {
            command = &alias.value;
        }
    }

    match command {
        "exit" => builtin_exit(&args),
        "help" => {builtin_help(&args); return},
        "cd" => {builtin_cd(&args); return},
        "alias" => {builtin_alias(&args, state); return},
        "prompt" => {builtin_prompt(&args, state); return},
        "set" => {builtin_set(&args, state); return},
        _ => {},
    }

    let result = Command::new(command)
        .args(args_1)
        .spawn();
    match result {
        Ok(mut child) => {
            let _ = child.wait();
        },
        Err(_e) => {
            println!("shell: '{}' does not exist", command);
        }
    }
}

fn load_config(state: &mut State) {
    let mut file = File::open(".conf").expect("Failed to open file");
    let mut contents = String::new();
    let _ = file.read_to_string(&mut contents);

    let commands: Vec<String> = contents.split("\n").map(|s| s.to_string()).collect();
    for cmd in commands.iter() {
        if cmd == "" {
            continue;
        }
        let args: Vec<String> = parse_input(&cmd.clone(), state);
        execute_command(&args, state);
    }
}

fn setup_env_variables(state: &mut State) {
    let username = whoami::username().expect("failed to get username");
    let hostname = whoami::hostname().expect("failed to get hostname");

    state.variables.push({ Variable {
        name: "USER".to_string(),
        value: username}});

    state.variables.push({ Variable {
        name: "HOST".to_string(),
        value: hostname}});
}

fn main() {
    let mut state = State {
        prompt: String::new(),
        aliases: Vec::new(),
        variables: Vec::new()};

    load_config(&mut state);
    setup_env_variables(&mut state);

    loop {
        print!("{}", prompt_to_string(&state));
        io::stdout().flush().unwrap();

        let input: String = read_input();
        let args: Vec<String> = parse_input(&input, &mut state);

        execute_command(&args, &mut state);
    }
}
