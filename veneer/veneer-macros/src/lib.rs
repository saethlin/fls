#![feature(proc_macro_diagnostic, proc_macro_span, proc_macro_quote)]
extern crate proc_macro;

use proc_macro::{quote, Delimiter, Diagnostic, Level, TokenStream, TokenTree};

#[proc_macro_attribute]
pub fn main(args: TokenStream, item: TokenStream) -> TokenStream {
    if !args.is_empty() {
        let start = args.clone().into_iter().next().unwrap().span();
        let end = args.into_iter().last().unwrap().span();
        let span = start.join(end).unwrap();
        Diagnostic::spanned(
            vec![span],
            Level::Error,
            "Attribute macro veneer_macros::main does not accept any arguments",
        )
        .emit();
    }

    let signature = item
        .clone()
        .into_iter()
        .take_while(|t| {
            if let TokenTree::Group(group) = t {
                group.delimiter() != Delimiter::Brace
            } else {
                true
            }
        })
        .collect::<Vec<_>>();

    let start = item.clone().into_iter().next().unwrap().span();
    let end = item.clone().into_iter().last().unwrap().span();
    let span = start.join(end).unwrap();
    let not_a_fn = Diagnostic::spanned(
        vec![span],
        Level::Error,
        "Attribute macro veneer_macros::main may only be applied to functions which take no arguments",
    );

    let name = if let (
        Some(TokenTree::Ident(f)),
        Some(TokenTree::Ident(name)),
        Some(TokenTree::Group(args)),
    ) = (signature.get(0), signature.get(1), signature.get(2))
    {
        if f.to_string() == "fn" && args.delimiter() == Delimiter::Parenthesis {
            name
        } else {
            not_a_fn.emit();
            return item;
        }
    } else {
        not_a_fn.emit();
        return item;
    };

    let name = TokenTree::from(name.clone());

    let header = if signature.len() == 3 {
        quote! {
            #[no_mangle]
            unsafe extern "C" fn __veneer_main() {
               $name();
               veneer::syscalls::exit(0);
            }
        }
    } else {
        quote! {
            #[no_mangle]
            unsafe extern "C" fn __veneer_main() {
               let exit_code = match $name() {
                    Ok(()) => 0,
                    Err(_) => 1,
                };
                veneer::syscalls::exit(exit_code);
            }
        }
    };
    header.into_iter().chain(item.into_iter()).collect()
}
