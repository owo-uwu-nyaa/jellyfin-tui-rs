use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::{
    AttrStyle, Data, DeriveInput, Expr, ExprLit, Fields, Ident, ItemStruct, Lit, LitStr, Meta,
    MetaNameValue, Variant, parse_macro_input, spanned::Spanned,
};

struct CommandVariant {
    variant: Variant,
    name: LitStr,
}

impl CommandVariant {
    fn new(variant: Variant) -> Result<Self, TokenStream> {
        if matches!(variant.fields, Fields::Unit) {
            let name = variant.attrs.iter().filter_map(|attr| {
                if let (AttrStyle::Outer, Some(ident)) = (attr.style, attr.path().get_ident()) {
                    if ident == "command" {
                        if let Meta::NameValue(MetaNameValue {
                            path: _,
                            eq_token: _,
                            value:
                                Expr::Lit(ExprLit {
                                    attrs: _,
                                    lit: Lit::Str(ref name),
                                }),
                        }) = attr.meta
                        {
                            Some(Ok(name.clone()))
                        }else{
                            Some(Err(quote_spanned! {attr.span()=> ::std::compile_error!("attribute must have form #[name = \"name\"]");}))
                        }
                    } else {
                        None
                    }
                } else{
                    None
                }
            }).next().unwrap_or_else(||{
                let name = variant.ident.to_string();
                let mut snake = String::new();
                //conversion slightly modified from serde
                for (i,c) in name.char_indices(){
                    if c.is_uppercase(){
                        if i!=0{
                        snake.push('-');
                        }
                    snake.push(c.to_ascii_lowercase());
                    }else{
                        snake.push(c);
                    }
                }
                Ok(LitStr::new(&snake, variant.ident.span()))
            })?;
            Ok(Self { variant, name })
        } else {
            Err(
                quote_spanned! {variant.span()=> ::std::compile_error!("Derive Macro only supports unit enum variants")},
            )
        }
    }

    fn pattern_to_name(&self, t: &Ident) -> TokenStream {
        let pattern = &self.variant.ident;
        let name = &self.name;
        quote! {#t::#pattern => #name}
    }
    fn pattern_from_name(&self, t: &Ident) -> TokenStream {
        let name = &self.name;
        let variant = &self.variant.ident;
        quote! {#name => ::std::option::Option::Some(#t::#variant)}
    }
}

#[proc_macro_derive(Command, attributes(command))]
pub fn command(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match input.data {
        Data::Struct(_) | Data::Union(_) => {
            quote_spanned! { input.span()=>
                             ::std::compile_error!("This derive macro only supports Enums");
            }
        }
        Data::Enum(e) => {
            let name = &input.ident;
            let variants: Vec<_> = e.variants.into_iter().map(CommandVariant::new).collect();
            let to_name_patterns = variants.iter().map(|variant| {
                variant
                    .as_ref()
                    .map(|variant| variant.pattern_to_name(name))
                    .unwrap_or_else(Clone::clone)
            });
            let from_name_patterns = variants.iter().map(|variant| {
                variant
                    .as_ref()
                    .map(|variant| variant.pattern_from_name(name))
                    .unwrap_or_else(Clone::clone)
            });
            quote! {
                impl ::keybinds::Command for #name {
                    fn to_name(self)->&'static str {
                        match self{
                            #(#to_name_patterns),*
                        }
                    }
                    fn from_name(name:&str)->::std::option::Option<Self>{
                        match name{
                            #(#from_name_patterns),* ,
                            _ => std::option::Option::None
                        }
                    }
                }
            }
        }
    }
    .into()
}

#[proc_macro_attribute]
pub fn gen_from_config(
    args: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as ItemStruct);
    if !args.is_empty(){
        quote_spanned! {input.span()=> ::std::compile_error!("from_config takes no arguments")}
    }else{
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
        quote_spanned! {input.span()=>
                        #input
                        impl #id {
            pub fn from_config(config: &::keybinds::parse_config::Config, strict: bool) -> ::keybinds::__eyre::Result<Self> {
                ::std::result::Result::Ok(Self{ #(#fields),* })
            }
        }}
    }.into()
}
