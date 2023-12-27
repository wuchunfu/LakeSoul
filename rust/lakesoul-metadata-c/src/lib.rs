// SPDX-FileCopyrightText: 2023 LakeSoul Contributors
//
// SPDX-License-Identifier: Apache-2.0

#![feature(c_size_t)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]
extern crate core;

use core::ffi::c_ptrdiff_t;
use std::ffi::{c_char, c_uchar, CStr, CString};
use std::io::Write;
use std::ptr::NonNull;

use lakesoul_metadata::{Builder, Client, MetaDataClient, PreparedStatementMap, Runtime};
use prost::bytes::BufMut;
use prost::Message;
use proto::proto::entity;

#[repr(C)]
pub struct Result<OpaqueT> {
    ptr: *mut OpaqueT,
    err: *const c_char,
}

impl<OpaqueT> Result<OpaqueT> {
    pub fn new<T>(obj: T) -> Self {
        Result {
            ptr: convert_to_opaque_raw::<T, OpaqueT>(obj),
            err: std::ptr::null(),
        }
    }

    pub fn error(err_msg: &str) -> Self {
        Result {
            ptr: std::ptr::null_mut(),
            err: CString::new(err_msg).unwrap().into_raw(),
        }
    }

    pub fn free<T>(&mut self) {
        unsafe {
            if !self.ptr.is_null() {
                drop(from_opaque::<OpaqueT, T>(NonNull::new_unchecked(self.ptr)));
            }
            if !self.err.is_null() {
                drop(CString::from_raw(self.err as *mut c_char));
            }
        }
    }
}

#[repr(C)]
pub struct PreparedStatement {
    private: [u8; 0],
}

#[repr(C)]
pub struct TokioPostgresClient {
    private: [u8; 0],
}

#[repr(C)]
pub struct TokioRuntime {
    private: [u8; 0],
}

#[repr(C)]
pub struct BytesResult {
    private: [u8; 0],
}

fn convert_to_opaque_raw<F, T>(obj: F) -> *mut T {
    Box::into_raw(Box::new(obj)) as *mut T
}

fn convert_to_nonnull<T>(obj: T) -> NonNull<T> {
    unsafe { NonNull::new_unchecked(Box::into_raw(Box::new(obj))) }
}

fn from_opaque<F, T>(obj: NonNull<F>) -> T {
    unsafe { *Box::from_raw(obj.as_ptr() as *mut T) }
}

fn from_nonnull<T>(obj: NonNull<T>) -> T {
    unsafe { *Box::from_raw(obj.as_ptr()) }
}

fn string_from_ptr(ptr: *const c_char) -> String {
    unsafe { CStr::from_ptr(ptr).to_str().unwrap().to_string() }
}

pub type ResultCallback<T> = extern "C" fn(T, *const c_char);

#[no_mangle]
pub extern "C" fn execute_insert(
    callback: extern "C" fn(i32, *const c_char),
    runtime: NonNull<Result<TokioRuntime>>,
    client: NonNull<Result<TokioPostgresClient>>,
    prepared: NonNull<Result<PreparedStatement>>,
    insert_type: i32,
    addr: c_ptrdiff_t,
    len: i32,
) {
    let runtime = unsafe { NonNull::new_unchecked(runtime.as_ref().ptr as *mut Runtime).as_ref() };
    let client = unsafe { NonNull::new_unchecked(client.as_ref().ptr as *mut Client).as_mut() };
    let prepared = unsafe { NonNull::new_unchecked(prepared.as_ref().ptr as *mut PreparedStatementMap).as_mut() };

    let raw_parts = unsafe { std::slice::from_raw_parts(addr as *const u8, len as usize) };
    let wrapper = entity::JniWrapper::decode(prost::bytes::Bytes::from(raw_parts)).unwrap();
    let result =
        runtime.block_on(async { lakesoul_metadata::execute_insert(client, prepared, insert_type, wrapper).await });
    match result {
        Ok(count) => callback(count, CString::new("").unwrap().into_raw()),
        Err(e) => callback(-1, CString::new(e.to_string().as_str()).unwrap().into_raw()),
    }
}

#[no_mangle]
pub extern "C" fn execute_update(
    callback: extern "C" fn(i32, *const c_char),
    runtime: NonNull<Result<TokioRuntime>>,
    client: NonNull<Result<TokioPostgresClient>>,
    prepared: NonNull<Result<PreparedStatement>>,
    update_type: i32,
    joined_string: *const c_char,
) {
    let runtime = unsafe { NonNull::new_unchecked(runtime.as_ref().ptr as *mut Runtime).as_ref() };
    let client = unsafe { NonNull::new_unchecked(client.as_ref().ptr as *mut Client).as_mut() };
    let prepared = unsafe { NonNull::new_unchecked(prepared.as_ref().ptr as *mut PreparedStatementMap).as_mut() };

    let result = runtime.block_on(async {
        lakesoul_metadata::execute_update(client, prepared, update_type, string_from_ptr(joined_string)).await
    });
    match result {
        Ok(count) => callback(count, CString::new("").unwrap().into_raw()),
        Err(e) => callback(-1, CString::new(e.to_string().as_str()).unwrap().into_raw()),
    }
}

#[no_mangle]
pub extern "C" fn execute_query_scalar(
    callback: extern "C" fn(*const c_char, *const c_char),
    runtime: NonNull<Result<TokioRuntime>>,
    client: NonNull<Result<TokioPostgresClient>>,
    prepared: NonNull<Result<PreparedStatement>>,
    update_type: i32,
    joined_string: *const c_char,
) {
    let runtime = unsafe { NonNull::new_unchecked(runtime.as_ref().ptr as *mut Runtime).as_ref() };
    let client = unsafe { NonNull::new_unchecked(client.as_ref().ptr as *mut Client).as_mut() };
    let prepared = unsafe { NonNull::new_unchecked(prepared.as_ref().ptr as *mut PreparedStatementMap).as_mut() };

    let result = runtime.block_on(async {
        lakesoul_metadata::execute_query_scalar(client, prepared, update_type, string_from_ptr(joined_string)).await
    });
    match result {
        Ok(Some(result)) => callback(
            CString::new(result.as_str()).unwrap().into_raw(),
            CString::new("").unwrap().into_raw(),
        ),
        Ok(None) => callback(
            CString::new("").unwrap().into_raw(),
            CString::new("").unwrap().into_raw(),
        ),
        Err(e) => callback(
            CString::new("").unwrap().into_raw(),
            CString::new(e.to_string().as_str()).unwrap().into_raw(),
        ),
    }
}

#[no_mangle]
pub extern "C" fn execute_query(
    callback: extern "C" fn(i32, *const c_char),
    runtime: NonNull<Result<TokioRuntime>>,
    client: NonNull<Result<TokioPostgresClient>>,
    prepared: NonNull<Result<PreparedStatement>>,
    query_type: i32,
    joined_string: *const c_char,
) -> NonNull<Result<BytesResult>> {
    let runtime = unsafe { NonNull::new_unchecked(runtime.as_ref().ptr as *mut Runtime).as_ref() };
    let client = unsafe { NonNull::new_unchecked(client.as_ref().ptr as *mut Client).as_ref() };
    let prepared = unsafe { NonNull::new_unchecked(prepared.as_ref().ptr as *mut PreparedStatementMap).as_mut() };

    let result = runtime.block_on(async {
        lakesoul_metadata::execute_query(client, prepared, query_type, string_from_ptr(joined_string)).await
    });
    match result {
        Ok(u8_vec) => {
            let len = u8_vec.len();
            callback(len as i32, CString::new("").unwrap().into_raw());
            convert_to_nonnull(Result::<BytesResult>::new::<Vec<u8>>(u8_vec))
        }
        Err(e) => {
            callback(-1, CString::new(e.to_string().as_str()).unwrap().into_raw());
            convert_to_nonnull(Result::<BytesResult>::new::<Vec<u8>>(vec![]))
        }
    }
}

#[no_mangle]
pub extern "C" fn export_bytes_result(
    callback: extern "C" fn(bool, *const c_char),
    bytes: NonNull<Result<BytesResult>>,
    len: i32,
    addr: c_ptrdiff_t,
) {
    let len = len as usize;
    let bytes = unsafe { NonNull::new_unchecked(bytes.as_ref().ptr as *mut Vec<c_uchar>).as_mut() };

    if bytes.len() != len {
        callback(
            false,
            CString::new("Size of buffer and result mismatch at export_bytes_result.")
                .unwrap()
                .into_raw(),
        );
        return;
    }
    bytes.push(0u8);
    bytes.shrink_to_fit();

    let dst = unsafe { std::slice::from_raw_parts_mut(addr as *mut u8, len + 1) };
    let mut writer = dst.writer();
    let _ = writer.write_all(bytes.as_slice());

    callback(true, CString::new("").unwrap().into_raw());
}

#[no_mangle]
pub extern "C" fn free_bytes_result(bytes: NonNull<Result<BytesResult>>) {
    from_nonnull(bytes).free::<Vec<u8>>();
}

#[no_mangle]
pub extern "C" fn clean_meta_for_test(
    callback: extern "C" fn(i32, *const c_char),
    runtime: NonNull<Result<TokioRuntime>>,
    client: NonNull<Result<TokioPostgresClient>>,
) {
    let runtime = unsafe { NonNull::new_unchecked(runtime.as_ref().ptr as *mut Runtime).as_ref() };
    let client = unsafe { NonNull::new_unchecked(client.as_ref().ptr as *mut Client).as_ref() };
    let result = runtime.block_on(async { lakesoul_metadata::clean_meta_for_test(client).await });
    match result {
        Ok(count) => callback(count, CString::new("").unwrap().into_raw()),
        Err(e) => callback(-1, CString::new(e.to_string().as_str()).unwrap().into_raw()),
    }
}

#[no_mangle]
pub extern "C" fn create_tokio_runtime() -> NonNull<Result<TokioRuntime>> {
    let runtime = Builder::new_multi_thread()
        .enable_all()
        .worker_threads(2)
        .max_blocking_threads(8)
        .build()
        .unwrap();
    convert_to_nonnull(Result::<TokioRuntime>::new(runtime))
}

#[no_mangle]
pub extern "C" fn free_tokio_runtime(runtime: NonNull<Result<TokioRuntime>>) {
    from_nonnull(runtime).free::<Runtime>();
}

#[no_mangle]
pub extern "C" fn create_tokio_postgres_client(
    callback: extern "C" fn(bool, *const c_char),
    config: *const c_char,
    runtime: NonNull<Result<TokioRuntime>>,
) -> NonNull<Result<TokioPostgresClient>> {
    let config = string_from_ptr(config);
    let runtime = unsafe { NonNull::new_unchecked(runtime.as_ref().ptr as *mut Runtime).as_ref() };

    let result = runtime.block_on(async { lakesoul_metadata::create_connection(config).await });

    let result = match result {
        Ok(client) => {
            callback(true, CString::new("").unwrap().into_raw());
            Result::<TokioPostgresClient>::new(client)
        }
        Err(e) => {
            callback(false, CString::new(e.to_string().as_str()).unwrap().into_raw());
            Result::<TokioPostgresClient>::error(format!("{}", e).as_str())
        }
    };
    convert_to_nonnull(result)
}

#[no_mangle]
pub extern "C" fn free_tokio_postgres_client(client: NonNull<Result<TokioPostgresClient>>) {
    from_nonnull(client).free::<Client>();
}

#[no_mangle]
pub extern "C" fn create_prepared_statement() -> NonNull<Result<PreparedStatement>> {
    let prepared = PreparedStatementMap::new();
    convert_to_nonnull(Result::<PreparedStatement>::new(prepared))
}

#[no_mangle]
pub extern "C" fn free_prepared_statement(prepared: NonNull<Result<PreparedStatement>>) {
    from_nonnull(prepared).free::<PreparedStatementMap>();
}

#[no_mangle]
pub extern "C" fn create_lakesoul_metadata_client() -> NonNull<Result<MetaDataClient>> {
    let client = MetaDataClient::from_env();
    convert_to_nonnull(Result::<MetaDataClient>::new(client))
}

#[no_mangle]
pub extern "C" fn free_lakesoul_metadata_client(prepared: NonNull<Result<MetaDataClient>>) {
    from_nonnull(prepared).free::<MetaDataClient>();
}
