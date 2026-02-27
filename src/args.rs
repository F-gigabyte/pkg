use std::env;

use crate::errors::PkgError;

pub struct Args<'a> {
    pub objcopy: String,
    pub ld: String,
    pub config_file: &'a str,
    pub debug: bool,
    pub outfile: &'a str
}

impl<'a> Args<'a> {
    pub fn parse(args: &'a Vec<String>) -> Result<Args<'a>, PkgError> {
        let objcopy = env::var("OBJCOPY").unwrap_or("objcopy".to_string());
        let ld = env::var("LD").unwrap_or("ld".to_string());
        let mut config_index = 1;
        if args.len() < 4 || args.len() > 5 {
            return Err(
                PkgError::InvalidArgs {
                    name: args[0].clone()
                }
            );
        }
        let debug = if args.len() == 5 {
            match args[1].as_ref() {
                "-r" => {
                    config_index = 2;
                    false
                },
                _ => {
                    return Err(
                        PkgError::InvalidArgs {
                            name: args[0].clone()
                        }
                    );
                }
            }
        } else {
            true
        };
        if args[config_index + 1] != "-o" {
            return Err(
                PkgError::InvalidArgs {
                    name: args[0].clone()
                }
            );
        }
        let outfile = &args[config_index + 2];
        let config_file = &args[config_index];
        Ok(Self { 
            objcopy, 
            ld, 
            config_file, 
            debug,
            outfile
        })
    }
}
