use std::{collections::{HashMap, VecDeque}, env::args};

pub struct Arguments {
    pub new_version: Option<String>,
    pub includes: Vec<String>,
}

pub fn parse_arguments() -> Arguments {
    let mut raw_arguments = args().skip(1).collect::<VecDeque<_>>();

    let mut positional_arguments = Vec::new();
    let mut non_positional_arguments = HashMap::new();

    while let Some(argument) = raw_arguments.pop_front() {
        if argument.starts_with("--") {
            let argument = argument.replacen("--", "", 1);

            if let Some((k, v)) = argument.split_once("=") {
                non_positional_arguments.insert(String::from(k), Some(String::from(v)));
            } else {
                non_positional_arguments.insert(argument, raw_arguments.pop_front());
            };

            break;
        }

        positional_arguments.push(argument);
    }

    Arguments {
        new_version: positional_arguments.pop(),
        includes: non_positional_arguments
            .iter()
            .filter_map(|(key, value)| {
                if key == "include" {
                    value.clone()
                } else {
                    None
                }
            })
            .collect::<Vec<_>>(),
    }
}
