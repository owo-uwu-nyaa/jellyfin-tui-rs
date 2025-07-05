use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::{
    Data, DeriveInput, Error, Fields, GenericParam, Ident, LitStr, Result, Type, Variant,
    parse_quote, parse2, spanned::Spanned,
};

#[derive(Debug, PartialEq, Eq)]
struct CommandVariant {
    ident: Ident,
    name: LitStr,
}

#[derive(Debug, PartialEq, Eq)]
struct FlattenVariant {
    ident: Ident,
    ty: Type,
}

#[derive(Debug, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
enum ParsedVariant {
    Command(CommandVariant),
    Flatten(FlattenVariant),
}

fn parse_variant(variant: Variant) -> Result<ParsedVariant> {
    let mut name: Option<LitStr> = None;
    let mut flatten = false;
    for attr in variant.attrs {
        if attr.path().is_ident("command") {
            attr.meta.require_list()?.parse_nested_meta(|meta| {
                if meta.path.is_ident("flatten") {
                    flatten = true;
                    if !meta.input.is_empty() {
                        Err(meta.error("flatten must be empty"))
                    } else {
                        Ok(())
                    }
                } else if meta.path.is_ident("name") {
                    name = Some(meta.value()?.parse()?);
                    Ok(())
                } else {
                    Err(meta.error("only \"flatten\" or \"name\" allowed"))
                }
            })?;
        }
    }
    if flatten {
        if variant.fields.len() == 1 {
            if let Fields::Unnamed(fields) = variant.fields {
                Ok(ParsedVariant::Flatten(FlattenVariant {
                    ident: variant.ident,
                    ty: fields.unnamed.first().unwrap().ty.clone(),
                }))
            } else {
                Err(Error::new(
                    variant.fields.span(),
                    "only tuple variants supported with flatten",
                ))
            }
        } else {
            Err(Error::new(
                variant.fields.span(),
                "the variant annotated with flatten must have exactly 1 field",
            ))
        }
    } else if let Fields::Unit = variant.fields {
        let name = name.unwrap_or_else(|| ident_to_kebab_lit(&variant.ident));
        Ok(ParsedVariant::Command(CommandVariant {
            ident: variant.ident,
            name,
        }))
    } else {
        Err(Error::new(
            variant.fields.span(),
            "variants not annotated with flatten must be unit variants",
        ))
    }
}

fn to_kebab_case(name: &str) -> String {
    let mut kebab = String::new();
    //conversion slightly modified from serde
    for (i, c) in name.char_indices() {
        if c.is_uppercase() {
            if i != 0 {
                kebab.push('-');
            }
            kebab.push(c.to_ascii_lowercase());
        } else {
            kebab.push(c);
        }
    }
    kebab
}

fn ident_to_kebab_lit(id: &Ident) -> LitStr {
    LitStr::new(&to_kebab_case(&id.to_string()), id.span())
}

impl CommandVariant {
    fn pattern_to_name(&self, t: &Ident) -> TokenStream {
        let pattern = &self.ident;
        let name = &self.name;
        quote! {#t::#pattern => #name}
    }
    fn pattern_from_name(&self, t: &Ident) -> TokenStream {
        let name = &self.name;
        let variant = &self.ident;
        quote! {#name => ::std::option::Option::Some(#t::#variant)}
    }
}

impl FlattenVariant {
    fn pattern_to_name(&self, t: &Ident) -> TokenStream {
        let pattern = &self.ident;
        quote_spanned! {self.ty.span()=> #t::#pattern(inner) => ::keybinds::Command::to_name(inner)}
    }
    fn gen_from_name(&self, t: &Ident, s: &Ident) -> TokenStream {
        let ty = &self.ty;
        let variant = &self.ident;
        let val = Ident::new("val", Span::mixed_site());
        quote_spanned! {self.ty.span()=> if let Some(#val)=<#ty as ::keybinds::Command>::from_name(#s){
            return ::std::option::Option::Some(#t::#variant(#val));
        }}
    }
}

fn gen_to_name(commands: &[CommandVariant], flattens: &[FlattenVariant], t: &Ident) -> TokenStream {
    let patterns = commands
        .iter()
        .map(|v| v.pattern_to_name(t))
        .chain(flattens.iter().map(|c| c.pattern_to_name(t)));
    quote! {
        fn to_name(self)->&'static str {
            match self{
                #(#patterns),*
            }
        }
    }
}

fn gen_from_name(
    commands: &[CommandVariant],
    flattens: &[FlattenVariant],
    t: &Ident,
) -> TokenStream {
    let commands = commands.iter().map(|c| c.pattern_from_name(t));
    let var = Ident::new_raw("str", Span::mixed_site());
    let flattens = flattens.iter().map(|f| f.gen_from_name(t, &var));
    quote! {
        fn from_name(name:&str)->::std::option::Option<Self>{
            match name{
                #(#commands),* ,
                #var => {
                    #(#flattens)*
                    ::std::option::Option::None
                }
            }
        }
    }
}

fn gen_all(commands: &[CommandVariant], flattens: &[FlattenVariant]) -> TokenStream {
    let commands = commands.iter().map(|c| &c.name);
    if flattens.is_empty() {
        quote! {
            fn all() -> &'static [&'static str] {
                const S: &'static [&'static str] = &[#(#commands),*];
                S
            }
        }
    } else {
        let flattens = flattens.iter().map(|f| &f.ty);
        quote! {
            fn all() -> &'static [&'static str] {
                static S: ::std::sync::LazyLock<&'static [&'static str]> = LazyLock::new(
                    || ::keybinds::__macro_support::collect_all_names(
                        &[
                            &[#(#commands),*],
                            #(<#flattens as ::keybinds::Command>::all()),*
                        ]
                    )
                );
                *S
            }
        }
    }
}

pub fn command(input: TokenStream) -> Result<TokenStream> {
    let input: DeriveInput = parse2(input)?;
    match input.data {
        Data::Struct(_) | Data::Union(_) => Err(Error::new(
            input.span(),
            "This derive macro only supports Enums",
        )),
        Data::Enum(e) => {
            let name = &input.ident;
            let mut generics = input.generics;
            for param in &mut generics.params {
                if let GenericParam::Type(ref mut type_param) = *param {
                    type_param.bounds.push(parse_quote!(::keybinds::Command))
                }
            }
            let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
            let mut commands = Vec::new();
            let mut flattens = Vec::new();
            let mut errors = Vec::new();
            for variant in e.variants.into_iter().map(parse_variant) {
                match variant {
                    Err(e) => errors.push(e),
                    Ok(ParsedVariant::Command(c)) => commands.push(c),
                    Ok(ParsedVariant::Flatten(f)) => flattens.push(f),
                }
            }
            commands.sort_by_key(|c| c.name.value());
            commands_unique(&commands, &mut errors);
            collect_errors(errors)?;
            let to_name = gen_to_name(&commands, &flattens, name);
            let from_name = gen_from_name(&commands, &flattens, name);
            let all = gen_all(&commands, &flattens);
            Ok(quote! {
                impl #impl_generics ::keybinds::Command for #name #ty_generics
                    #where_clause
                {
                    #to_name
                    #from_name
                    #all
                }
            })
        }
    }
}

fn collect_errors(errors: Vec<Error>) -> Result<()> {
    match errors.into_iter().reduce(|mut a, b| {
        a.combine(b);
        a
    }) {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

fn commands_unique(commands: &[CommandVariant], errors: &mut Vec<Error>) {
    let mut iter = commands.iter();
    if let Some(mut last) = iter.next() {
        for current in iter {
            if last.name == current.name {
                errors.push(Error::new(
                    current.name.span(),
                    format!("identifier \"{}\" is used twice", current.name.value()),
                ));
            }
            last = current;
        }
    }
}

#[cfg(test)]
mod tests {
    use quote::quote;
    use syn::{ItemImpl, parse_quote, parse2};

    use super::{CommandVariant, FlattenVariant, ParsedVariant, Result, command, parse_variant};
    use pretty_assertions::assert_eq;
    #[test]
    fn test_generate() -> Result<()> {
        let gen_impl: ItemImpl = parse2(command(quote! {
            enum Test<T,T2: Command>{
                #[command(name = "name")]
                A,
                ValB,
                #[command(flatten)]
                Rec(I),
                #[command(flatten)]
                Rec2(T),
                #[command(flatten)]
                Rec3(T2),
            }
        })?)?;
        let expected_impl: ItemImpl = parse2(quote! {
            impl<T: ::keybinds::Command,T2: Command + ::keybinds::Command> ::keybinds::Command for Test<T,T2>{
                fn to_name(self)->&'static str {
                    match self{
                        Test::A => "name",
                        Test::ValB => "val-b",
                        Test::Rec(inner) => ::keybinds::Command::to_name(inner),
                        Test::Rec2(inner) => ::keybinds::Command::to_name(inner),
                        Test::Rec3(inner) => ::keybinds::Command::to_name(inner)
                    }
                }
                fn from_name(name:&str)->::std::option::Option<Self>{
                    match name{
                        "name" => ::std::option::Option::Some(Test::A),
                        "val-b" => ::std::option::Option::Some(Test::ValB),
                        r#str => {
                            if let Some(val)=<I as ::keybinds::Command>::from_name(r#str){
                                return ::std::option::Option::Some(Test::Rec(val));
                            }
                            if let Some(val)=<T as ::keybinds::Command>::from_name(r#str){
                                return ::std::option::Option::Some(Test::Rec2(val));
                            }
                            if let Some(val)=<T2 as ::keybinds::Command>::from_name(r#str){
                                return ::std::option::Option::Some(Test::Rec3(val));
                            }
                            ::std::option::Option::None
                        }
                    }
                }
                fn all() -> &'static [&'static str] {
                    static S: ::std::sync::LazyLock<&'static [&'static str]> = LazyLock::new(
                        || ::keybinds::__macro_support::collect_all_names(
                            &[
                                &["name", "val-b"],
                                <I as ::keybinds::Command>::all(),
                                <T as ::keybinds::Command>::all(),
                                <T2 as ::keybinds::Command>::all()
                            ]
                        )
                    );
                    *S
                }
            }
        })?;
        assert_eq!(expected_impl, gen_impl);
        Ok(())
    }

    #[test]
    fn test_variant_flatten() -> Result<()> {
        let parsed = parse_variant(parse_quote! {
                #[command(flatten)]
                Rec(I)
        })?;
        let expected = ParsedVariant::Flatten(FlattenVariant {
            ident: parse_quote!(Rec),
            ty: parse_quote!(I),
        });
        assert_eq!(expected, parsed);
        Ok(())
    }

    #[test]
    fn test_variant_command() -> Result<()> {
        let parsed = parse_variant(parse_quote!(CMDOneTwo))?;
        let expected = ParsedVariant::Command(CommandVariant {
            ident: parse_quote!(CMDOneTwo),
            name: parse_quote!("c-m-d-one-two"),
        });
        assert_eq!(expected, parsed);
        Ok(())
    }

    #[test]
    fn test_variant_command_named() -> Result<()> {
        let parsed = parse_variant(parse_quote!(
            #[command(name = "testName")]
            Cmd
        ))?;
        let expected = ParsedVariant::Command(CommandVariant {
            ident: parse_quote!(Cmd),
            name: parse_quote!("testName"),
        });
        assert_eq!(expected, parsed);
        Ok(())
    }
}
