use dioform::Form;

// Neither the model nor its field value implements the generated namespace's traits.
struct CustomValue(String);

#[derive(Form)]
struct ProfileForm {
    value: CustomValue,
}

fn assert_namespace_traits<T: Copy + Clone + std::fmt::Debug + Default>() {}

fn main() {
    assert_namespace_traits::<ProfileFormFields>();

    let fields = ProfileForm::fields();
    let first = move || fields.value();
    let second = move || fields.value();
    let model = ProfileForm {
        value: CustomValue("Ada".to_owned()),
    };

    assert_eq!(first().get(&model).0, "Ada");
    assert_eq!(second().get(&model).0, "Ada");

    let default_fields = ProfileFormFields::default();
    assert_eq!(default_fields.value().get(&model).0, "Ada");
}
