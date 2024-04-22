use embedded_graphics::{
    geometry::OriginDimensions,
    image::{ImageDrawable, ImageDrawableExt},
};
use once_cell::sync::Lazy;
use std::collections::HashMap;

use crate::gui::{primitives::Color, render::ImageBuffer};

// ASCII subset
const SUPPORTED_CHARACTERS: [char; 95] = [
    '@', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R',
    'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', '[', '\\', ']', '^', '_', '`', 'a', 'b', 'c', 'd', 'e',
    'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x',
    'y', 'z', '{', '|', '}', '~', ' ', '!', '"', '#', '$', '%', '&', '\'', '(', ')', '*', '+', ',',
    '-', '.', '/', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', ':', ';', '<', '=', '>', '?',
];

pub static FONT_6X9: Lazy<Font> =
    Lazy::new(|| Font::from_embedded_graphics_font(&embedded_graphics::mono_font::ascii::FONT_6X9));

#[derive(Clone)]
pub struct Character {
    pub image_buffer: ImageBuffer,
}

#[derive(Clone, Copy)]
pub struct Font {
    characters: &'static HashMap<char, Character>,
    pub letter_spacing: u32,
}

impl Font {
    pub fn get_character(&self, c: char) -> &Character {
        self.characters
            .get(&c)
            .or_else(|| self.characters.get(&'?'))
            .unwrap()
    }

    /// Warning: This leaks memory, only use it to create static instances.
    fn from_embedded_graphics_font(font: &'static embedded_graphics::mono_font::MonoFont) -> Self {
        let mut characters = Box::new(HashMap::new());

        for c in SUPPORTED_CHARACTERS.iter() {
            let sub_image = glyph(font, *c);
            let size = sub_image.size();

            let mut draw_target = InMemoryDrawTarget::new(size.width, size.height);
            sub_image.draw(&mut draw_target).unwrap();

            let character = Character {
                image_buffer: ImageBuffer {
                    width: size.width as usize,
                    height: size.height as usize,
                    pixels: draw_target.buffer,
                },
            };

            characters.insert(*c, character);
        }

        return Font {
            characters: Box::leak(characters),
            letter_spacing: font.character_spacing as u32,
        };
    }
}

// Glyph fn from embedded_graphics::mono_font::MonoFont

fn glyph<'a>(
    font: &'a embedded_graphics::mono_font::MonoFont,
    c: char,
) -> embedded_graphics::image::SubImage<
    'a,
    embedded_graphics::image::ImageRaw<'a, embedded_graphics::pixelcolor::BinaryColor>,
> {
    let glyphs_per_row = font.image.size().width / font.character_size.width;

    // Char _code_ offset from first char, most often a space
    // E.g. first char = ' ' (32), target char = '!' (33), offset = 33 - 32 = 1
    let glyph_index = font.glyph_mapping.index(c) as u32;
    let row = glyph_index / glyphs_per_row;

    // Top left corner of character, in pixels
    let char_x = (glyph_index - (row * glyphs_per_row)) * font.character_size.width;
    let char_y = row * font.character_size.height;

    font.image
        .sub_image(&embedded_graphics::primitives::Rectangle::new(
            embedded_graphics::geometry::Point::new(char_x as i32, char_y as i32),
            font.character_size,
        ))
}

/// Compatibility with embedded_graphics
struct InMemoryDrawTarget {
    buffer: Vec<Color>,
    width: u32,
    height: u32,
}

impl InMemoryDrawTarget {
    fn new(width: u32, height: u32) -> Self {
        InMemoryDrawTarget {
            buffer: vec![Color::black(); (width * height) as usize],
            width,
            height,
        }
    }

    fn point_to_index(&self, point: embedded_graphics::geometry::Point) -> Option<usize> {
        if point.x >= 0
            && point.x < self.width as i32
            && point.y >= 0
            && point.y < self.height as i32
        {
            Some((point.y as u32 * self.width + point.x as u32) as usize)
        } else {
            None
        }
    }
}

impl embedded_graphics::draw_target::DrawTarget for InMemoryDrawTarget {
    type Color = embedded_graphics::pixelcolor::BinaryColor;
    type Error = ();

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = embedded_graphics::Pixel<Self::Color>>,
    {
        for embedded_graphics::Pixel(point, color) in pixels.into_iter() {
            if let Some(index) = self.point_to_index(point) {
                self.buffer[index] = color.into();
            }
        }

        Ok(())
    }
}

impl OriginDimensions for InMemoryDrawTarget {
    fn size(&self) -> embedded_graphics::geometry::Size {
        embedded_graphics::geometry::Size::new(self.width, self.height)
    }
}

impl From<embedded_graphics::pixelcolor::BinaryColor> for Color {
    fn from(color: embedded_graphics::pixelcolor::BinaryColor) -> Self {
        let color_value = match color {
            embedded_graphics::pixelcolor::BinaryColor::On => 255,
            embedded_graphics::pixelcolor::BinaryColor::Off => 0,
        };
        Color {
            r: color_value,
            g: color_value,
            b: color_value,
            a: 255,
        }
    }
}
