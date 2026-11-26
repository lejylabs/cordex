//! Optional derive macros for `cordex` (`macros` feature).
//!
//! Provides [`CommandChoice`] and [`CommandArgs`] without forcing proc-macros
//! on users who prefer the macro-free builder API.

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Data, DeriveInput, Expr, Fields, GenericArgument, Lit, Meta, PathArguments, Type,
    parse_macro_input,
};

/// Derive `cordex_interactions::CommandChoice` for a unit enum.
///
/// Each variant becomes a Discord string choice. Override the display name or
/// wire value with `#[choice(name = "...", value = "...")]`.
#[proc_macro_derive(CommandChoice, attributes(choice))]
pub fn derive_command_choice(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand_command_choice(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Derive `cordex_interactions::CommandArgs` for a struct with named fields.
///
/// Supported field types (and `Option<T>` variants): `String`, `i64`, `f64`,
/// `bool`, `UserId`, `ChannelId`, `RoleId`, or any `CommandChoice` enum
/// annotated with `#[arg(choice)]`.
///
/// Set option descriptions with `#[arg(description = "...")]` or doc comments.
#[proc_macro_derive(CommandArgs, attributes(arg))]
pub fn derive_command_args(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand_command_args(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn expand_command_choice(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let ident = &input.ident;
    let Data::Enum(data_enum) = &input.data else {
        return Err(syn::Error::new_spanned(
            ident,
            "CommandChoice can only be derived for enums",
        ));
    };

    let mut choice_exprs = Vec::with_capacity(data_enum.variants.len());
    let mut match_arms = Vec::with_capacity(data_enum.variants.len());

    for variant in &data_enum.variants {
        if !matches!(variant.fields, Fields::Unit) {
            return Err(syn::Error::new_spanned(
                variant,
                "CommandChoice variants must be unit variants",
            ));
        }
        let v_ident = &variant.ident;
        let mut display_name = v_ident.to_string();
        let mut wire_value = display_name.to_lowercase();

        for attr in &variant.attrs {
            if attr.path().is_ident("choice") {
                attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("name") {
                        let val: Lit = meta.value()?.parse()?;
                        if let Lit::Str(s) = val {
                            display_name = s.value();
                        }
                    } else if meta.path.is_ident("value") {
                        let val: Lit = meta.value()?.parse()?;
                        if let Lit::Str(s) = val {
                            wire_value = s.value();
                        }
                    }
                    Ok(())
                })?;
            }
        }

        choice_exprs.push(quote! {
            ::cordex_interactions::Choice::string(#display_name, #wire_value)
        });
        match_arms.push(quote! {
            #wire_value => ::core::option::Option::Some(Self::#v_ident)
        });
    }

    Ok(quote! {
        impl ::cordex_interactions::CommandChoice for #ident {
            fn choices() -> ::std::vec::Vec<::cordex_interactions::Choice> {
                ::std::vec![#(#choice_exprs),*]
            }

            fn from_choice_value(value: &str) -> ::core::option::Option<Self> {
                match value {
                    #(#match_arms,)*
                    _ => ::core::option::Option::None,
                }
            }
        }
    })
}

#[allow(clippy::too_many_lines)]
fn expand_command_args(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let ident = &input.ident;
    let Data::Struct(data_struct) = &input.data else {
        return Err(syn::Error::new_spanned(
            ident,
            "CommandArgs can only be derived for structs",
        ));
    };
    let Fields::Named(named) = &data_struct.fields else {
        return Err(syn::Error::new_spanned(
            ident,
            "CommandArgs requires a struct with named fields",
        ));
    };

    let mut option_exprs = Vec::with_capacity(named.named.len());
    let mut field_inits = Vec::with_capacity(named.named.len());

    for field in &named.named {
        let Some(f_ident) = &field.ident else {
            continue;
        };
        let mut opt_name = f_ident.to_string();
        let mut description = extract_doc_comment(&field.attrs).unwrap_or_else(|| opt_name.clone());
        let mut is_choice = false;

        for attr in &field.attrs {
            if attr.path().is_ident("arg") {
                attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("name") {
                        let val: Lit = meta.value()?.parse()?;
                        if let Lit::Str(s) = val {
                            opt_name = s.value();
                        }
                    } else if meta.path.is_ident("description") {
                        let val: Lit = meta.value()?.parse()?;
                        if let Lit::Str(s) = val {
                            description = s.value();
                        }
                    } else if meta.path.is_ident("choice") {
                        is_choice = true;
                    }
                    Ok(())
                })?;
            }
        }

        let (inner_ty, is_opt) = unwrap_option(&field.ty);
        let req_suffix = if is_opt {
            quote! {}
        } else {
            quote! { .required() }
        };

        if is_choice {
            option_exprs.push(quote! {
                ::cordex_interactions::CommandOption::string(#opt_name, #description)
                    .choices(<#inner_ty as ::cordex_interactions::CommandChoice>::choices())
                    #req_suffix
            });
            if is_opt {
                field_inits.push(quote! {
                    #f_ident: match options.string_opt(#opt_name)? {
                        ::core::option::Option::Some(raw) => {
                            ::core::option::Option::Some(
                                <#inner_ty as ::cordex_interactions::CommandChoice>::from_choice_value(raw)
                                    .ok_or_else(|| ::cordex_interactions::InteractionError::InvalidCommand(
                                        ::std::format!("invalid choice `{raw}` for option `{}`", #opt_name).into()
                                    ))?
                            )
                        }
                        ::core::option::Option::None => ::core::option::Option::None,
                    }
                });
            } else {
                field_inits.push(quote! {
                    #f_ident: {
                        let raw = options.string(#opt_name)?;
                        <#inner_ty as ::cordex_interactions::CommandChoice>::from_choice_value(raw)
                            .ok_or_else(|| ::cordex_interactions::InteractionError::InvalidCommand(
                                ::std::format!("invalid choice `{raw}` for option `{}`", #opt_name).into()
                            ))?
                    }
                });
            }
            continue;
        }

        let type_name = last_segment_name(inner_ty).unwrap_or_default();
        let (opt_ctor, parse_expr) = match type_name.as_str() {
            "String" => (
                quote! { ::cordex_interactions::CommandOption::string(#opt_name, #description) #req_suffix },
                if is_opt {
                    quote! { options.string_opt(#opt_name)?.map(::std::borrow::ToOwned::to_owned) }
                } else {
                    quote! { options.string(#opt_name)?.to_owned() }
                },
            ),
            "i64" => (
                quote! { ::cordex_interactions::CommandOption::integer(#opt_name, #description) #req_suffix },
                if is_opt {
                    quote! { options.integer_opt(#opt_name)? }
                } else {
                    quote! { options.integer(#opt_name)? }
                },
            ),
            "f64" => (
                quote! { ::cordex_interactions::CommandOption::number(#opt_name, #description) #req_suffix },
                if is_opt {
                    quote! { options.number_opt(#opt_name)? }
                } else {
                    quote! { options.number(#opt_name)? }
                },
            ),
            "bool" => (
                quote! { ::cordex_interactions::CommandOption::boolean(#opt_name, #description) #req_suffix },
                if is_opt {
                    quote! { options.boolean_opt(#opt_name)? }
                } else {
                    quote! { options.boolean(#opt_name)? }
                },
            ),
            "UserId" => (
                quote! { ::cordex_interactions::CommandOption::user(#opt_name, #description) #req_suffix },
                if is_opt {
                    quote! { options.user_id_opt(#opt_name)? }
                } else {
                    quote! { options.user_id(#opt_name)? }
                },
            ),
            "ChannelId" => (
                quote! { ::cordex_interactions::CommandOption::channel(#opt_name, #description) #req_suffix },
                if is_opt {
                    quote! { options.channel_id_opt(#opt_name)? }
                } else {
                    quote! { options.channel_id(#opt_name)? }
                },
            ),
            "RoleId" => (
                quote! { ::cordex_interactions::CommandOption::role(#opt_name, #description) #req_suffix },
                if is_opt {
                    quote! { options.role_id_opt(#opt_name)? }
                } else {
                    quote! { options.role_id(#opt_name)? }
                },
            ),
            _ => {
                return Err(syn::Error::new_spanned(
                    inner_ty,
                    "unsupported CommandArgs field type (expected String, i64, f64, bool, UserId, ChannelId, RoleId, or #[arg(choice)])",
                ));
            }
        };

        option_exprs.push(opt_ctor);
        field_inits.push(quote! { #f_ident: #parse_expr });
    }

    Ok(quote! {
        impl ::cordex_interactions::CommandArgs for #ident {
            fn options() -> ::std::vec::Vec<::cordex_interactions::CommandOption> {
                ::std::vec![#(#option_exprs),*]
            }

            fn from_options(
                options: &::cordex_interactions::CommandOptions<'_>,
            ) -> ::core::result::Result<Self, ::cordex_interactions::InteractionError> {
                ::core::result::Result::Ok(Self {
                    #(#field_inits,)*
                })
            }
        }
    })
}

fn unwrap_option(ty: &Type) -> (&Type, bool) {
    if let Type::Path(tp) = ty
        && let Some(seg) = tp.path.segments.last()
        && seg.ident == "Option"
        && let PathArguments::AngleBracketed(args) = &seg.arguments
        && let Some(GenericArgument::Type(inner)) = args.args.first()
    {
        return (inner, true);
    }
    (ty, false)
}

fn last_segment_name(ty: &Type) -> Option<String> {
    if let Type::Path(tp) = ty {
        tp.path.segments.last().map(|s| s.ident.to_string())
    } else {
        None
    }
}

fn extract_doc_comment(attrs: &[syn::Attribute]) -> Option<String> {
    for attr in attrs {
        if attr.path().is_ident("doc")
            && let Meta::NameValue(nv) = &attr.meta
            && let Expr::Lit(expr_lit) = &nv.value
            && let Lit::Str(s) = &expr_lit.lit
        {
            let trimmed = s.value().trim().to_owned();
            if !trimmed.is_empty() {
                return Some(trimmed);
            }
        }
    }
    None
}
