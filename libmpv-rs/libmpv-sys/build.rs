// Copyright (C) 2016  ParadoxSpiral
//
// This file is part of mpv-sys.
//
// This library is free software; you can redistribute it and/or
// modify it under the terms of the GNU Lesser General Public
// License as published by the Free Software Foundation; either
// version 2.1 of the License, or (at your option) any later version.
//
// This library is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// Lesser General Public License for more details.
//
// You should have received a copy of the GNU Lesser General Public
// License along with this library; if not, write to the Free Software
// Foundation, Inc., 51 Franklin Street, Fifth Floor, Boston, MA  02110-1301  USA

#[cfg(feature = "use-bindgen")]
use std::env;
#[cfg(feature = "use-bindgen")]
use std::path::PathBuf;

#[cfg(feature = "use-bindgen")]
fn gen_bindings(include_paths: &[PathBuf]) {
    let bindings = bindgen::Builder::default()
        .clang_args(
            include_paths
                .iter()
                .map(|path| format!("-I{}", path.to_string_lossy())),
        )
        .header_contents(
            "combined.h",
            r"
#include <mpv/client.h>
#include <mpv/render.h>
#include <mpv/render_gl.h>
#include <mpv/stream_cb.h>
",
        )
        .impl_debug(true)
        .opaque_type("mpv_handle")
        .opaque_type("mpv_render_context")
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");

    println!("cargo:rustc-link-lib=mpv");
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let _libmpv = pkg_config::Config::new()
        .atleast_version("2.3.0")
        .probe("mpv")
        .unwrap();
    #[cfg(feature = "use-bindgen")]
    gen_bindings(&_libmpv.include_paths);
}
