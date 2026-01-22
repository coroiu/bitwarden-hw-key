use embedded_graphics::{pixelcolor::BinaryColor, Drawable};

use crate::credentials::Credential;
use crate::simple_gui::{
    components::{ComponentAction, VerticalMenu, VerticalMenuItem},
    font,
    primitives::Rectangle,
    Canvas, Document, StandaloneView,
};

pub fn create_view(width: u32, height: u32) -> Document {
    create_credential_list_view(&[], width, height)
}

/// Create a standalone view for the credential list
fn create_credential_list_standalone_view(
    credentials: &[Credential],
    width: u32,
    height: u32,
) -> StandaloneView {
    let mut view = StandaloneView::new(width, height);
    let mut menu = VerticalMenu::new(Rectangle::new(0, 0, width, height), &font::FONT_5X8);

    if credentials.is_empty() {
        // Show empty state (unfocusable items)
        menu.items_mut()
            .push(VerticalMenuItem::new(&font::FONT_5X8, "No credentials").focusable(false));
        menu.items_mut()
            .push(VerticalMenuItem::new(&font::FONT_5X8, "Sync from vault").focusable(false));
    } else {
        // Show credentials with navigation to detail view
        for cred in credentials {
            // Format: "Name (username)"
            let label = if cred.username.is_empty() {
                cred.name.clone()
            } else {
                format!("{} ({})", cred.name, cred.username)
            };

            // Clone credential for the closure
            let cred_clone = cred.clone();
            let item = VerticalMenuItem::new(&font::FONT_5X8, &label).on_activate(move || {
                // Clone again for the view builder closure
                let cred_for_builder = cred_clone.clone();
                ComponentAction::PushView(Box::new(move || {
                    create_credential_detail_standalone_view(&cred_for_builder, width, height)
                }))
            });
            menu.items_mut().push(item);
        }
    }

    view.components.push(Box::new(menu));
    view
}

pub fn create_credential_list_view(credentials: &[Credential], width: u32, height: u32) -> Document {
    let mut document = Document::new(width, height);
    let view = create_credential_list_standalone_view(credentials, width, height);

    // Replace the default view with our credential list view
    *document.components_mut() = view.components;

    document
}

/// Create a standalone view for credential detail
fn create_credential_detail_standalone_view(
    credential: &Credential,
    width: u32,
    height: u32,
) -> StandaloneView {
    let mut view = StandaloneView::new(width, height);
    let mut menu = VerticalMenu::new(Rectangle::new(0, 0, width, height), &font::FONT_5X8);

    // Back button at the top
    let back_item = VerticalMenuItem::new(&font::FONT_5X8, "< Back")
        .on_activate(|| ComponentAction::PopView);
    menu.items_mut().push(back_item);

    // Separator
    menu.items_mut()
        .push(VerticalMenuItem::new(&font::FONT_5X8, "---").focusable(false));

    // Title - credential name (focusable to allow scrolling)
    menu.items_mut()
        .push(VerticalMenuItem::new(&font::FONT_5X8, &credential.name));

    // Separator
    menu.items_mut()
        .push(VerticalMenuItem::new(&font::FONT_5X8, "----------------").focusable(false));

    // Username field
    if !credential.username.is_empty() {
        let username_label = format!("User: {}", credential.username);
        menu.items_mut()
            .push(VerticalMenuItem::new(&font::FONT_5X8, &username_label));
    }

    // Password field (hidden)
    if !credential.password.is_empty() {
        menu.items_mut()
            .push(VerticalMenuItem::new(&font::FONT_5X8, "Pass: ••••••••"));
    }

    // URI field
    if let Some(uri) = &credential.uri {
        if !uri.is_empty() {
            let uri_label = format!("URL: {}", uri);
            menu.items_mut()
                .push(VerticalMenuItem::new(&font::FONT_5X8, &uri_label));
        }
    }

    // Notes field
    if let Some(notes) = &credential.notes {
        if !notes.is_empty() {
            menu.items_mut()
                .push(VerticalMenuItem::new(&font::FONT_5X8, "---").focusable(false));
            let notes_label = format!("Note: {}", notes);
            menu.items_mut()
                .push(VerticalMenuItem::new(&font::FONT_5X8, &notes_label));
        }
    }

    view.components.push(Box::new(menu));
    view
}

pub fn create_credential_detail_view(credential: &Credential, width: u32, height: u32) -> Document {
    let mut document = Document::new(width, height);
    let view = create_credential_detail_standalone_view(credential, width, height);

    // Replace the default view with our credential detail view
    *document.components_mut() = view.components;

    document
}

impl Drawable for Canvas {
    type Color = BinaryColor;
    type Output = ();

    fn draw<D>(&self, target: &mut D) -> Result<Self::Output, D::Error>
    where
        D: embedded_graphics::prelude::DrawTarget<Color = Self::Color>,
    {
        let pixels = self
            .image_buffer
            .pixels
            .iter()
            .enumerate()
            .map(|(i, color)| {
                let x = i % self.image_buffer.width;
                let y = i / self.image_buffer.width;

                let combined_colors = color.r as i32 + color.g as i32 + color.b as i32;
                let mapped_color = match combined_colors {
                    c if c > 300 => BinaryColor::On,
                    _ => BinaryColor::Off,
                };

                embedded_graphics::Pixel(
                    embedded_graphics::geometry::Point::new(x as i32, y as i32),
                    mapped_color,
                )
            });

        target.draw_iter(pixels)
    }
}
