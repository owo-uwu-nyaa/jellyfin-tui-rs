use proc_macro2::TokenStream;
use quote::quote_spanned;
use syn::{Error, Field, ItemStruct, LitStr, Result, parse2, spanned::Spanned};

pub fn keybind_config(args: TokenStream, input: TokenStream) -> Result<TokenStream> {
    let input: ItemStruct = parse2(input)?;
    if !args.is_empty() {
        Err(Error::new(input.span(), "from_config takes no arguments"))
    } else {
        let id = &input.ident;
        let fields = input.fields.iter().map(|field|{
            if let Some(ref ident)=field.ident{
                let context = "in map ".to_string()+&ident.to_string();
                let context = LitStr::new(&context, ident.span());
                quote_spanned! {field.span()=>
                                #ident: ::keybinds::__eyre::WrapErr::context(config.parse(::std::stringify!(#ident), strict), #context)?
                }
            }else{
                quote_spanned! {field.span()=> ::std::compile_error!("from config requires a non tuple struct")}
            }
        });
        let uniqueness = input.fields.iter().map(assert_uniqueness);
        Ok(quote_spanned! {input.span()=>
                           #input
                           impl #id {
                               pub fn from_config(config: &::keybinds::parse_config::Config, strict: bool) -> ::keybinds::__eyre::Result<Self> {
                                   ::std::result::Result::Ok(Self{ #(#fields),* })
                               }
                               #[allow(dead_code)]
                               pub fn assert_uniqueness(){
                                   #(#uniqueness)*
                               }
                           }

        })
    }
}

pub fn assert_uniqueness(field: &Field) -> TokenStream {
    let ty = &field.ty;
    quote_spanned! {field.span()=>
        ::keybinds::__macro_support::commands_unique(<<#ty as ::keybinds::__macro_support::BindingMapExt>::T as ::keybinds::Command>::all(), ::std::stringify!(#ty));
    }
}
