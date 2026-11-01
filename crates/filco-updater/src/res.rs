//! Win32 resource access: read an RT_RCDATA resource from an exe mapped as data (no code of it runs), and
//! replace one in a copy of an exe with UpdateResourceW.

use std::os::windows::ffi::OsStrExt as _;
use std::path::Path;

use windows::Win32::Foundation::{FreeLibrary, HMODULE};
use windows::Win32::System::LibraryLoader::{
    BeginUpdateResourceW, EndUpdateResourceW, FindResourceExW, LOAD_LIBRARY_AS_DATAFILE,
    LOAD_LIBRARY_AS_IMAGE_RESOURCE, LoadLibraryExW, LoadResource, LockResource, SizeofResource,
    UpdateResourceW,
};
use windows::core::PCWSTR;

use crate::Error;

const RT_RCDATA: u16 = 10;
const LANG: u16 = 0;

fn wide(p: &Path) -> Vec<u16> {
    p.as_os_str().encode_wide().chain(std::iter::once(0)).collect()
}

/// MAKEINTRESOURCEW: an integer id passed where the API takes a name.
fn int_resource(id: u16) -> PCWSTR {
    PCWSTR(usize::from(id) as *const u16)
}

fn win(e: &windows::core::Error, what: &str) -> Error {
    Error::Resource(format!("{what}: {e}"))
}

struct Module(HMODULE);

impl Drop for Module {
    fn drop(&mut self) {
        // SAFETY: the handle came from LoadLibraryExW and is freed exactly once, here.
        let _ = unsafe { FreeLibrary(self.0) };
    }
}

/// The bytes of RT_RCDATA resource `id` (language neutral) in the exe at `path`.
pub fn read(path: &Path, id: u16) -> Result<Vec<u8>, Error> {
    let name = wide(&std::path::absolute(path).map_err(|e| Error::Io(e.to_string()))?);
    // SAFETY: `name` is a NUL-terminated wide string that outlives the call. LOAD_LIBRARY_AS_DATAFILE |
    // LOAD_LIBRARY_AS_IMAGE_RESOURCE maps the file as data: no entry point or TLS callback runs.
    let module = unsafe {
        LoadLibraryExW(
            PCWSTR(name.as_ptr()),
            None,
            LOAD_LIBRARY_AS_DATAFILE | LOAD_LIBRARY_AS_IMAGE_RESOURCE,
        )
    }
    .map_err(|e| win(&e, "LoadLibraryExW"))?;
    let module = Module(module);
    // SAFETY: the module handle is valid while `module` lives; the type and id are integer resources.
    let found = unsafe { FindResourceExW(Some(module.0), int_resource(RT_RCDATA), int_resource(id), LANG) };
    if found.is_invalid() {
        return Err(Error::Resource(format!("resource RT_RCDATA {id} not found")));
    }
    // SAFETY: `found` was returned by FindResourceExW for this module.
    let size = unsafe { SizeofResource(Some(module.0), found) } as usize;
    // SAFETY: as above; the resource memory belongs to the mapped module.
    let global = unsafe { LoadResource(Some(module.0), found) }.map_err(|e| win(&e, "LoadResource"))?;
    // SAFETY: `global` is the loaded resource of a module still mapped; LockResource returns its address.
    let ptr = unsafe { LockResource(global) }.cast::<u8>();
    if ptr.is_null() {
        return Err(Error::Resource(format!(
            "resource RT_RCDATA {id} could not be locked"
        )));
    }
    // SAFETY: the resource is `size` bytes at `ptr`, mapped read-only until `module` is dropped after the copy.
    let bytes = unsafe { std::slice::from_raw_parts(ptr, size) }.to_vec();
    Ok(bytes)
}

/// Replaces RT_RCDATA resource `id` of the exe at `path` (a copy, never the stock file) with `data`.
pub fn write(path: &Path, id: u16, data: &[u8]) -> Result<(), Error> {
    let name = wide(&std::path::absolute(path).map_err(|e| Error::Io(e.to_string()))?);
    let len = u32::try_from(data.len()).map_err(|_| Error::Resource("resource too large".into()))?;
    // SAFETY: `name` is a NUL-terminated wide string that outlives the call.
    let h = unsafe { BeginUpdateResourceW(PCWSTR(name.as_ptr()), false) }
        .map_err(|e| win(&e, "BeginUpdateResourceW"))?;
    // SAFETY: `h` is the update handle just opened; `data` is `len` readable bytes that outlive the call, which
    // copies them.
    let updated = unsafe {
        UpdateResourceW(
            h,
            int_resource(RT_RCDATA),
            int_resource(id),
            LANG,
            Some(data.as_ptr().cast()),
            len,
        )
    };
    if let Err(e) = updated {
        // SAFETY: discard the pending update; the handle is not used afterwards.
        let _ = unsafe { EndUpdateResourceW(h, true) };
        return Err(win(&e, "UpdateResourceW"));
    }
    // SAFETY: commits and closes the handle, used exactly once.
    unsafe { EndUpdateResourceW(h, false) }.map_err(|e| win(&e, "EndUpdateResourceW"))
}
