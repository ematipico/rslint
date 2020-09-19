use proc_macro2::TokenStream;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::*;
use unindent::unindent;

pub fn parse_group_mod(file: &str) -> Result<Group> {
    let file = parse_file(file)?;
    for item in file.items.iter() {
        if let Item::Macro(macro_call) = item {
            let call = macro_call.mac.clone();
            if call
                .path
                .segments
                .last()
                .map_or(false, |x| x.ident == "group")
            {
                return parse2::<Group>(call.tokens);
            }
        }
    }
    Err(Error::new_spanned(
        file,
        "Expected a group! declaration in group mod file",
    ))
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Group {
    pub name: String,
    pub docstring: String,
}

impl Parse for Group {
    fn parse(input: ParseStream) -> Result<Self> {
        let docstring = parse_docstring(&input).unwrap_or_default();
        let name = input.parse::<Ident>()?.to_string();
        let _ = input.parse::<TokenStream>();
        Ok(Self { name, docstring })
    }
}

pub fn parse_rule_file(file: &str) -> Result<Option<RuleFile>> {
    let file = parse_file(file)?;
    let mut tests = None;
    let mut declaration = None;
    for item in file.items {
        if let Item::Macro(macro_call) = item {
            let call = macro_call.mac;
            if call
                .path
                .segments
                .last()
                .map_or(false, |x| x.ident == "declare_lint")
            {
                let res = parse2::<LintDeclaration>(call.tokens);
                declaration = Some(res?);
            } else if call
                .path
                .segments
                .last()
                .map_or(false, |x| x.ident == "rule_tests")
            {
                tests = Some(parse2::<RuleTests>(call.tokens)?);
            }
        }
    }

    if let Some(decl) = declaration {
        Ok(Some(RuleFile {
            lint_declaration: decl,
            tests,
        }))
    } else {
        Ok(None)
    }
}

#[derive(Clone)]
pub struct RuleFile {
    pub lint_declaration: LintDeclaration,
    pub tests: Option<RuleTests>,
}

/// A single `declare_lint!` declaration.
#[derive(Clone)]
pub struct LintDeclaration {
    pub name: String,
    pub docstring: Option<String>,
    pub config_fields: Vec<ConfigField>,
}

#[derive(Clone)]
pub struct ConfigField {
    pub docstring: Option<String>,
    pub field: Field,
}

impl Parse for LintDeclaration {
    fn parse(input: ParseStream) -> Result<Self> {
        let docstring = parse_docstring(input);
        input.parse::<Ident>()?;
        input.parse::<Token!(,)>()?;
        input.parse::<Ident>()?;
        input.parse::<Token!(,)>()?;
        let name = input.parse::<LitStr>()?.value();
        let _ = input.parse::<Token!(,)>();

        let config_fields = Punctuated::<ConfigField, Token![,]>::parse_terminated(input)?
            .into_iter()
            .filter(|field| matches!(field.field.vis, Visibility::Public(_)))
            .collect();
            
        Ok(Self {
            name,
            docstring,
            config_fields,
        })
    }
}

impl Parse for ConfigField {
    fn parse(input: ParseStream) -> Result<Self> {
        let docstring = parse_docstring(input);
        let field = input.call(Field::parse_named)?;
        Ok(Self { docstring, field })
    }
}

fn parse_docstring(input: ParseStream) -> Option<String> {
    let mut res = String::new();
    while input.peek(Token!(#)) {
        if let Ok(attr) = input.call(Attribute::parse_outer) {
            for attribute in attr {
                if attribute
                    .path
                    .get_ident()
                    .map_or(false, |ident| ident == "doc")
                {
                    let tokens = attribute.tokens.clone().into_iter().skip(1).collect();
                    let string = parse2::<LitStr>(tokens).expect("Invalid docstring").value();
                    res.push_str(&string);
                    res.push('\n');
                }
            }
        }
    }
    if res.is_empty() {
        None
    } else {
        Some(unindent(&res))
    }
}

mod kw {
    syn::custom_keyword!(err);
    syn::custom_keyword!(ok);
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RuleTests {
    pub ok_examples: Vec<Example>,
    pub err_examples: Vec<Example>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Example {
    pub source: String,
    pub docstring: Option<String>,
}

impl Parse for RuleTests {
    fn parse(input: ParseStream) -> Result<Self> {
        input.parse::<Expr>()?;
        input.parse::<Token!(,)>()?;
        input.parse::<kw::err>()?;
        input.parse::<Token!(:)>()?;
        let content;
        braced!(content in input);
        let mut err_examples: Vec<Example> =
            Punctuated::<Example, Token![,]>::parse_terminated(&content)?
                .into_iter()
                .collect();
        err_examples = err_examples
            .into_iter()
            .filter(|elem| elem.docstring.as_ref().map(|x| x.trim()) != Some("ignore"))
            .collect();

        err_examples.truncate(30);

        input.parse::<Token!(,)>()?;
        input.parse::<kw::ok>()?;
        input.parse::<Token!(:)>()?;
        let content;
        braced!(content in input);
        let mut ok_examples: Vec<Example> =
            Punctuated::<Example, Token![,]>::parse_terminated(&content)?
                .into_iter()
                .collect();
        ok_examples = ok_examples
            .into_iter()
            .filter(|elem| elem.docstring.as_ref().map(|x| x.trim()) != Some("ignore"))
            .collect();

        ok_examples.truncate(30);

        Ok(Self {
            ok_examples,
            err_examples,
        })
    }
}

impl Parse for Example {
    fn parse(input: ParseStream) -> Result<Self> {
        let docstring = parse_docstring(&input);
        let source = unindent(&input.parse::<LitStr>()?.value())
            .trim()
            .to_string();
        Ok(Example {
            docstring,
            source,
        })
    }
}
