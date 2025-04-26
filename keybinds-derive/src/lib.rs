
#[proc_macro_derive(Command, attributes(command))]
pub fn command(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    keybinds_derive_impl::command(input.into()).unwrap_or_else(keybinds_derive_impl::Error::into_compile_error)
    .into()
}

#[proc_macro_attribute]
pub fn gen_from_config(
    args: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    keybinds_derive_impl::gen_from_config(args.into(), input.into()).unwrap_or_else(keybinds_derive_impl::Error::into_compile_error).into()
    }
