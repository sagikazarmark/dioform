use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

use crate::contract::{DeriveKind, NamedFormStructContract};

#[proc_macro_derive(Form, attributes(form))]
pub fn derive_form(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match expand_form(input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

#[proc_macro_derive(FieldGroup, attributes(form))]
pub fn derive_field_group(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match expand_field_group(input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn expand_form(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    Ok(NamedFormStructContract::from_derive_input(input, DeriveKind::Form)?.expand())
}

fn expand_field_group(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    Ok(
        NamedFormStructContract::from_derive_input(input, DeriveKind::FieldGroup)?
            .expand_field_group(),
    )
}

mod contract {
    use quote::{format_ident, quote};
    use syn::{Data, DeriveInput, Field, Fields, LitStr, Visibility};

    pub(super) struct NamedFormStructContract {
        model_ident: syn::Ident,
        fields_ident: syn::Ident,
        visibility: Visibility,
        crate_path: syn::Path,
        static_fields: Vec<StaticFieldPathContract>,
    }

    #[derive(Clone, Copy)]
    pub(super) enum DeriveKind {
        Form,
        FieldGroup,
    }

    impl DeriveKind {
        fn derive_name(self) -> &'static str {
            match self {
                Self::Form => "Form",
                Self::FieldGroup => "FieldGroup",
            }
        }

        fn generic_subject(self) -> &'static str {
            match self {
                Self::Form => "form models",
                Self::FieldGroup => "field groups",
            }
        }

        fn generic_error(self) -> String {
            format!(
                "#[derive({})] does not support generic {} yet",
                self.derive_name(),
                self.generic_subject()
            )
        }

        fn named_struct_error(self) -> String {
            format!(
                "#[derive({})] supports named structs only",
                self.derive_name()
            )
        }

        fn enum_model_error(self) -> String {
            match self {
                Self::Form => "#[derive(Form)] does not support enum form models; use a named struct root and treat enum values as Variant Fields until variant-inner traversal has explicit semantics".to_owned(),
                Self::FieldGroup => self.named_struct_error(),
            }
        }

        fn tuple_struct_error(self) -> String {
            match self {
                Self::Form => "#[derive(Form)] does not support tuple form models; use a named struct root because tuple field positions do not provide stable form field vocabulary".to_owned(),
                Self::FieldGroup => self.named_struct_error(),
            }
        }

        fn unit_struct_error(self) -> String {
            match self {
                Self::Form => "#[derive(Form)] does not support unit form models; add named fields so the form can derive addressable Field Paths".to_owned(),
                Self::FieldGroup => self.named_struct_error(),
            }
        }

        fn union_model_error(self) -> String {
            match self {
                Self::Form => "#[derive(Form)] does not support union form models; use a named struct root with safe addressable fields".to_owned(),
                Self::FieldGroup => self.named_struct_error(),
            }
        }
    }

    impl NamedFormStructContract {
        pub(super) fn from_derive_input(
            input: DeriveInput,
            derive_kind: DeriveKind,
        ) -> syn::Result<Self> {
            let options = FormModelOptions::from_attrs(&input.attrs)?;

            if !input.generics.params.is_empty() {
                return Err(syn::Error::new_spanned(
                    input.generics,
                    derive_kind.generic_error(),
                ));
            }

            let model_ident = input.ident;
            let fields_ident = format_ident!("{}Fields", model_ident);
            let visibility = input.vis;

            let fields = match input.data {
                Data::Struct(data) => match data.fields {
                    Fields::Named(fields) => fields.named,
                    Fields::Unnamed(fields) => {
                        return Err(syn::Error::new_spanned(
                            fields,
                            derive_kind.tuple_struct_error(),
                        ));
                    }
                    Fields::Unit => {
                        return Err(syn::Error::new_spanned(
                            &model_ident,
                            derive_kind.unit_struct_error(),
                        ));
                    }
                },
                Data::Enum(data) => {
                    return Err(syn::Error::new_spanned(
                        data.enum_token,
                        derive_kind.enum_model_error(),
                    ));
                }
                Data::Union(data) => {
                    return Err(syn::Error::new_spanned(
                        data.union_token,
                        derive_kind.union_model_error(),
                    ));
                }
            };

            let mut static_fields = Vec::new();

            for field in fields {
                if let Some(field) =
                    StaticFieldPathContract::from_named_field(field, options.rename_all.as_ref())?
                {
                    static_fields.push(field);
                }
            }

            reject_duplicate_field_names(&static_fields)?;

            Ok(Self {
                model_ident,
                fields_ident,
                visibility,
                crate_path: options.crate_path,
                static_fields,
            })
        }

        pub(super) fn expand(&self) -> proc_macro2::TokenStream {
            let model_ident = &self.model_ident;
            let fields_ident = &self.fields_ident;
            let visibility = &self.visibility;
            let crate_path = &self.crate_path;
            let accessors = self
                .static_fields
                .iter()
                .map(|field| field.expand_accessor(model_ident, crate_path));

            quote! {
                #visibility struct #fields_ident;

                impl #fields_ident {
                    #(#accessors)*
                }

                impl #crate_path::Form for #model_ident {
                    type Fields = #fields_ident;

                    fn fields() -> Self::Fields {
                        #fields_ident
                    }
                }
            }
        }

        pub(super) fn expand_field_group(&self) -> proc_macro2::TokenStream {
            let model_ident = &self.model_ident;
            let group_map_ident = format_ident!("{}FieldGroupMap", model_ident);
            let visibility = &self.visibility;
            let crate_path = &self.crate_path;
            let map_fields = self
                .static_fields
                .iter()
                .map(|field| field.expand_group_map_field(crate_path));
            let accessors = self
                .static_fields
                .iter()
                .map(|field| field.expand_group_map_accessor(crate_path));
            let mounted_fields = self
                .static_fields
                .iter()
                .map(|field| field.expand_mounted_group_field(model_ident, crate_path));
            let clone_fields = self
                .static_fields
                .iter()
                .map(StaticFieldPathContract::expand_group_map_clone_field);

            quote! {
                #visibility struct #group_map_ident<Model> {
                    #(#map_fields,)*
                }

                impl<Model> Clone for #group_map_ident<Model> {
                    fn clone(&self) -> Self {
                        Self {
                            #(#clone_fields,)*
                        }
                    }
                }

                impl<Model> #group_map_ident<Model> {
                    #(#accessors)*
                }

                impl #crate_path::FieldGroup for #model_ident {
                    type Map<Model> = #group_map_ident<Model>;

                    fn mount<Model>(prefix: #crate_path::FieldPath<Model, Self>) -> Self::Map<Model>
                    where
                        Model: 'static,
                        Self: 'static,
                    {
                        #group_map_ident {
                            #(#mounted_fields,)*
                        }
                    }
                }
            }
        }
    }

    struct StaticFieldPathContract {
        field_ident: syn::Ident,
        field_ty: syn::Type,
        visibility: Visibility,
        identity: String,
        field_name: String,
        field_name_span: proc_macro2::Span,
    }

    impl StaticFieldPathContract {
        fn from_named_field(
            field: Field,
            rename_all: Option<&RenameRule>,
        ) -> syn::Result<Option<Self>> {
            let Field {
                attrs,
                ident,
                ty,
                vis,
                ..
            } = field;
            let options = FormFieldOptions::from_attrs(&attrs)?;

            if options.skip {
                return Ok(None);
            }

            let field_ident =
                ident.ok_or_else(|| syn::Error::new_spanned(&ty, "expected a named field"))?;
            let identity = field_ident.to_string();
            let (field_name, field_name_span) = options.name.unwrap_or_else(|| {
                let name =
                    rename_all.map_or_else(|| identity.clone(), |rule| rule.rename(&identity));

                (name, field_ident.span())
            });

            Ok(Some(Self {
                field_ident,
                field_ty: ty,
                visibility: vis,
                identity,
                field_name,
                field_name_span,
            }))
        }

        fn expand_accessor(
            &self,
            model_ident: &syn::Ident,
            crate_path: &syn::Path,
        ) -> proc_macro2::TokenStream {
            let field_ident = &self.field_ident;
            let field_ty = &self.field_ty;
            let visibility = &self.visibility;
            let identity = &self.identity;
            let field_name = &self.field_name;

            quote! {
                #visibility fn #field_ident(&self) -> #crate_path::FieldPath<#model_ident, #field_ty> {
                    #crate_path::FieldPath::direct(
                        #crate_path::FieldIdentity::new(#identity),
                        #field_name,
                        |model: &#model_ident| &model.#field_ident,
                        |model: &mut #model_ident| &mut model.#field_ident,
                    )
                }
            }
        }

        fn expand_group_map_field(&self, crate_path: &syn::Path) -> proc_macro2::TokenStream {
            let field_ident = &self.field_ident;
            let field_ty = &self.field_ty;
            let visibility = &self.visibility;

            quote! {
                #visibility #field_ident: #crate_path::FieldPath<Model, #field_ty>
            }
        }

        fn expand_group_map_accessor(&self, crate_path: &syn::Path) -> proc_macro2::TokenStream {
            let field_ident = &self.field_ident;
            let field_ty = &self.field_ty;
            let visibility = &self.visibility;

            quote! {
                #visibility fn #field_ident(&self) -> #crate_path::FieldPath<Model, #field_ty> {
                    self.#field_ident.clone()
                }
            }
        }

        fn expand_group_map_clone_field(&self) -> proc_macro2::TokenStream {
            let field_ident = &self.field_ident;

            quote! {
                #field_ident: self.#field_ident.clone()
            }
        }

        fn expand_mounted_group_field(
            &self,
            model_ident: &syn::Ident,
            crate_path: &syn::Path,
        ) -> proc_macro2::TokenStream {
            let field_ident = &self.field_ident;
            let identity = &self.identity;
            let field_name = &self.field_name;

            quote! {
                #field_ident: prefix.clone().join(#crate_path::FieldPath::direct(
                    #crate_path::FieldIdentity::new(#identity),
                    #field_name,
                    |model: &#model_ident| &model.#field_ident,
                    |model: &mut #model_ident| &mut model.#field_ident,
                ))
            }
        }
    }

    fn reject_duplicate_field_names(fields: &[StaticFieldPathContract]) -> syn::Result<()> {
        let mut seen: Vec<&StaticFieldPathContract> = Vec::new();

        for field in fields {
            for previous in &seen {
                if previous.field_name == field.field_name {
                    return Err(syn::Error::new(
                        field.field_name_span,
                        format!(
                            "duplicate rendered field name `{}` for fields `{}` and `{}`",
                            field.field_name, previous.identity, field.identity
                        ),
                    ));
                }
            }

            seen.push(field);
        }

        Ok(())
    }

    struct FormModelOptions {
        crate_path: syn::Path,
        rename_all: Option<RenameRule>,
    }

    impl Default for FormModelOptions {
        fn default() -> Self {
            Self {
                crate_path: syn::parse_quote!(::dioform),
                rename_all: None,
            }
        }
    }

    impl FormModelOptions {
        fn from_attrs(attrs: &[syn::Attribute]) -> syn::Result<Self> {
            let mut options = Self::default();

            for attr in attrs {
                if !attr.path().is_ident("form") {
                    continue;
                }

                attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("crate") {
                        let value = meta.value()?;
                        let value = value.parse::<LitStr>()?;
                        options.crate_path = value.parse()?;
                        return Ok(());
                    }

                    if meta.path.is_ident("rename_all") {
                        let value = meta.value()?;
                        let value = value.parse::<LitStr>()?;
                        options.rename_all = Some(RenameRule::from_lit(&value)?);
                        return Ok(());
                    }

                    Err(meta.error("unsupported #[form(...)] model attribute"))
                })?;
            }

            Ok(options)
        }
    }

    #[derive(Clone, Copy)]
    enum RenameRule {
        CamelCase,
    }

    impl RenameRule {
        fn from_lit(value: &LitStr) -> syn::Result<Self> {
            match value.value().as_str() {
                "camelCase" => Ok(Self::CamelCase),
                _ => Err(syn::Error::new_spanned(
                    value,
                    "unsupported #[form(rename_all = ...)] policy",
                )),
            }
        }

        fn rename(self, field_ident: &str) -> String {
            match self {
                Self::CamelCase => to_camel_case(field_ident),
            }
        }
    }

    fn to_camel_case(field_ident: &str) -> String {
        let mut output = String::with_capacity(field_ident.len());
        let mut uppercase_next = false;

        for character in field_ident.chars() {
            if character == '_' {
                uppercase_next = true;
                continue;
            }

            if uppercase_next {
                output.extend(character.to_uppercase());
                uppercase_next = false;
            } else {
                output.push(character);
            }
        }

        output
    }

    #[derive(Default)]
    struct FormFieldOptions {
        name: Option<(String, proc_macro2::Span)>,
        skip: bool,
    }

    impl FormFieldOptions {
        fn from_attrs(attrs: &[syn::Attribute]) -> syn::Result<Self> {
            let mut options = Self::default();

            for attr in attrs {
                if !attr.path().is_ident("form") {
                    continue;
                }

                attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("skip") {
                        options.skip = true;
                        return Ok(());
                    }

                    if meta.path.is_ident("name") {
                        let value = meta.value()?;
                        let value = value.parse::<LitStr>()?;
                        options.name = Some((value.value(), value.span()));
                        return Ok(());
                    }

                    Err(meta.error("unsupported #[form(...)] attribute"))
                })?;
            }

            Ok(options)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use syn::parse_quote;

        fn contract_error_for(
            input: DeriveInput,
            derive_kind: DeriveKind,
            message: &str,
        ) -> syn::Error {
            match NamedFormStructContract::from_derive_input(input, derive_kind) {
                Ok(_) => panic!("{message}"),
                Err(error) => error,
            }
        }

        fn contract_error(input: DeriveInput, message: &str) -> syn::Error {
            contract_error_for(input, DeriveKind::Form, message)
        }

        #[test]
        fn static_fields_use_rust_field_identifiers_as_field_identity() {
            let input: DeriveInput = parse_quote! {
                struct ProfileForm {
                    #[form(name = "contact-email")]
                    email: String,
                    #[form(name = "accepted_terms")]
                    accepts_terms: bool,
                }
            };

            let contract = NamedFormStructContract::from_derive_input(input, DeriveKind::Form)
                .expect("named form struct should be supported");

            assert_eq!(contract.static_fields[0].identity, "email");
            assert_eq!(contract.static_fields[0].field_name, "contact-email");
            assert_eq!(contract.static_fields[1].identity, "accepts_terms");
            assert_eq!(contract.static_fields[1].field_name, "accepted_terms");
        }

        #[test]
        fn skipped_fields_are_not_static_field_paths() {
            let input: DeriveInput = parse_quote! {
                struct AccountForm {
                    email: String,
                    #[form(skip)]
                    internal_token: String,
                }
            };

            let contract = NamedFormStructContract::from_derive_input(input, DeriveKind::Form)
                .expect("named form struct should be supported");

            assert_eq!(contract.static_fields.len(), 1);
            assert_eq!(contract.static_fields[0].identity, "email");
        }

        #[test]
        fn generic_models_fail_clearly() {
            let input: DeriveInput = parse_quote! {
                struct GenericForm<T> {
                    value: T,
                }
            };

            let error = contract_error(input, "generic forms are unsupported");

            assert_eq!(
                error.to_string(),
                "#[derive(Form)] does not support generic form models yet"
            );
        }

        #[test]
        fn generic_field_groups_fail_clearly() {
            let input: DeriveInput = parse_quote! {
                struct GenericGroup<T> {
                    value: T,
                }
            };

            let error = contract_error_for(
                input,
                DeriveKind::FieldGroup,
                "generic field groups are unsupported",
            );

            assert_eq!(
                error.to_string(),
                "#[derive(FieldGroup)] does not support generic field groups yet"
            );
        }

        #[test]
        fn model_attributes_fail_clearly() {
            let input: DeriveInput = parse_quote! {
                #[form(unknown = "value")]
                struct RenamedForm {
                    first_name: String,
                }
            };

            let error = contract_error(input, "model attributes are unsupported");

            assert_eq!(
                error.to_string(),
                "unsupported #[form(...)] model attribute"
            );
        }

        #[test]
        fn model_crate_attribute_overrides_generated_paths() {
            let input: DeriveInput = parse_quote! {
                #[form(crate = "::renamed_form")]
                struct RenamedForm {
                    first_name: String,
                }
            };

            let contract = NamedFormStructContract::from_derive_input(input, DeriveKind::Form)
                .expect("crate path should be supported");
            let crate_path = &contract.crate_path;

            assert_eq!(quote::quote!(#crate_path).to_string(), ":: renamed_form");
        }

        #[test]
        fn tuple_structs_fail_clearly() {
            let input: DeriveInput = parse_quote! {
                struct TupleForm(String);
            };

            let error = contract_error(input, "tuple forms are unsupported");

            assert_eq!(
                error.to_string(),
                "#[derive(Form)] does not support tuple form models; use a named struct root because tuple field positions do not provide stable form field vocabulary"
            );
        }

        #[test]
        fn tuple_field_groups_fail_clearly() {
            let input: DeriveInput = parse_quote! {
                struct TupleGroup(String);
            };

            let error = contract_error_for(
                input,
                DeriveKind::FieldGroup,
                "tuple field groups are unsupported",
            );

            assert_eq!(
                error.to_string(),
                "#[derive(FieldGroup)] supports named structs only"
            );
        }

        #[test]
        fn unit_structs_fail_clearly() {
            let input: DeriveInput = parse_quote! {
                struct UnitForm;
            };

            let error = contract_error(input, "unit forms are unsupported");

            assert_eq!(
                error.to_string(),
                "#[derive(Form)] does not support unit form models; add named fields so the form can derive addressable Field Paths"
            );
        }

        #[test]
        fn enums_fail_clearly() {
            let input: DeriveInput = parse_quote! {
                enum FormChoice {
                    Login,
                    Signup,
                }
            };

            let error = contract_error(input, "enum forms are unsupported");

            assert_eq!(
                error.to_string(),
                "#[derive(Form)] does not support enum form models; use a named struct root and treat enum values as Variant Fields until variant-inner traversal has explicit semantics"
            );
        }

        #[test]
        fn unions_fail_clearly() {
            let input: DeriveInput = parse_quote! {
                union UnsafeForm {
                    text: std::mem::ManuallyDrop<String>,
                }
            };

            let error = contract_error(input, "union forms are unsupported");

            assert_eq!(
                error.to_string(),
                "#[derive(Form)] does not support union form models; use a named struct root with safe addressable fields"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn skipped_fields_do_not_generate_accessors() {
        let input: DeriveInput = parse_quote! {
            struct AccountForm {
                email: String,
                #[form(skip)]
                internal_token: String,
            }
        };

        let output = expand_form(input).expect("form should expand").to_string();

        assert!(output.contains("email"));
        assert!(!output.contains("internal_token"));
    }

    #[test]
    fn expand_form_emits_the_facade_form_impl() {
        let input: DeriveInput = parse_quote! {
            pub struct SignupForm {
                email: String,
            }
        };

        let output = expand_form(input).expect("form should expand").to_string();

        assert!(output.contains("pub struct SignupFormFields"));
        assert!(output.contains("impl :: dioform :: Form for SignupForm"));
    }

    #[test]
    fn accessors_mirror_field_visibility() {
        let input: DeriveInput = parse_quote! {
            pub struct AccountForm {
                secret: String,
                pub email: String,
            }
        };

        let output = expand_form(input).expect("form should expand").to_string();

        assert!(output.contains("fn secret"));
        assert!(!output.contains("pub fn secret"));
        assert!(output.contains("pub fn email"));
    }
}
