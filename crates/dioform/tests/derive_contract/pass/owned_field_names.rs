use dioform::Form;
use dioxus::prelude::*;

#[derive(Form)]
#[form(rename_all = "camelCase")]
struct ProfileForm {
    first_name: String,
    #[form(name = "email-address")]
    email: String,
}

#[derive(Form)]
struct SignupForm {
    profile: ProfileForm,
}

fn main() {
    let name = ProfileForm::fields().first_name().field_name_owned();
    assert_eq!(&*name, "firstName");
    let email = ProfileForm::fields().email().field_name_owned();
    assert_eq!(&*email, "email-address");
    let nested = SignupForm::fields()
        .profile()
        .join(ProfileForm::fields().email())
        .field_name_owned();
    assert_eq!(&*nested, "profile.email-address");

    let _: Element = rsx! {
        input { name: ProfileForm::fields().first_name().field_name_owned() }
        input { name: ProfileForm::fields().email().field_name_owned() }
        input {
            name: SignupForm::fields()
                .profile()
                .join(ProfileForm::fields().email())
                .field_name_owned(),
        }
    };
}
