// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::error::Error;

use dbus;
use dbus::arg::{ArgType, Iter, IterAppend};
use dbus::tree::{MethodErr, MTFn, PropInfo};

use engine::{EngineError, ErrorEnum};

use super::types::{DbusErrorEnum, TData};

pub const STRATIS_BASE_PATH: &str = "/org/storage/stratis1";
pub const STRATIS_BASE_SERVICE: &str = "org.storage.stratis1";

/// Convert a tuple as option to an Option type
pub fn tuple_to_option<T>(value: (bool, T)) -> Option<T> {
    if value.0 { Some(value.1) } else { None }
}

/// Get the next argument off the bus
pub fn get_next_arg<'a, T>(iter: &mut Iter<'a>, loc: u16) -> Result<T, MethodErr>
    where T: dbus::arg::Get<'a> + dbus::arg::Arg
{
    if iter.arg_type() == ArgType::Invalid {
        return Err(MethodErr::no_arg());
    };
    let value: T = iter.read::<T>()
        .map_err(|_| MethodErr::invalid_arg(&loc))?;
    Ok(value)
}


/// Translates an engine error to the (errorcode, string) tuple that Stratis
/// D-Bus methods return.
pub fn engine_to_dbus_err_tuple(err: &EngineError) -> (u16, String) {
    #![allow(match_same_arms)]
    let error = match *err {
        EngineError::Engine(ref e, _) => {
            match *e {
                ErrorEnum::Error => DbusErrorEnum::ERROR,
                ErrorEnum::AlreadyExists => DbusErrorEnum::ALREADY_EXISTS,
                ErrorEnum::Busy => DbusErrorEnum::BUSY,
                ErrorEnum::Invalid => DbusErrorEnum::ERROR,
                ErrorEnum::NotFound => DbusErrorEnum::NOTFOUND,
            }
        }
        EngineError::Io(_) => DbusErrorEnum::IO_ERROR,
        EngineError::Nix(_) => DbusErrorEnum::NIX_ERROR,
        EngineError::Uuid(_) => DbusErrorEnum::INTERNAL_ERROR,
        EngineError::Utf8(_) => DbusErrorEnum::INTERNAL_ERROR,
        EngineError::Serde(_) => DbusErrorEnum::INTERNAL_ERROR,
        EngineError::DM(_) => DbusErrorEnum::INTERNAL_ERROR,
    };
    (error.into(), err.description().to_owned())
}

/// Convenience function to get the error value for "OK"
pub fn msg_code_ok() -> u16 {
    DbusErrorEnum::OK.into()
}

/// Convenience function to get the error string for "OK"
pub fn msg_string_ok() -> String {
    DbusErrorEnum::OK.get_error_string().to_owned()
}

/// Get the UUID for an object path.
pub fn get_uuid(i: &mut IterAppend, p: &PropInfo<MTFn<TData>, TData>) -> Result<(), MethodErr> {
    let object_path = p.path.get_name();
    let path = p.tree
        .get(object_path)
        .expect("implicit argument must be in tree");

    let data =
        path.get_data()
            .as_ref()
            .ok_or_else(|| MethodErr::failed(&format!("no data for object path {}", object_path)))?;

    i.append(format!("{}", data.uuid.simple()));
    Ok(())
}


/// Get the parent object path for an object path.
pub fn get_parent(i: &mut IterAppend, p: &PropInfo<MTFn<TData>, TData>) -> Result<(), MethodErr> {
    let object_path = p.path.get_name();
    let path = p.tree
        .get(object_path)
        .expect("implicit argument must be in tree");

    let data =
        path.get_data()
            .as_ref()
            .ok_or_else(|| MethodErr::failed(&format!("no data for object path {}", object_path)))?;

    i.append(data.parent.clone());
    Ok(())
}
