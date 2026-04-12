/* 
 * Copyright 2026 Fraser Griffin
 *
 * This file is part of Pkg.
 *
 * Pkg is free software: you can redistribute it and/or modify it under 
 * the terms of the GNU General Public License as published by the Free Software Foundation, 
 * either version 3 of the License, or (at your option) any later version.
 *
 * Pkg is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; 
 * without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. 
 * See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU Lesser General Public License along with Pkg. 
 * If not, see <https://www.gnu.org/licenses/>. 
 * 
 */

use std::process::Command;

/// Runs a command and checks the result for error  
/// `cmd` is the command to run  
/// On error, returns the error code as an `i32`
pub fn check_cmd(mut cmd: Command) -> Result<(), i32> {
    let status = cmd.output().map_err(|_| 0)?.status.code().ok_or(0)?;
    if  status == 0 {
        Ok(())
    } else {
        Err(status)
    }
}
