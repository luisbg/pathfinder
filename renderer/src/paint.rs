// pathfinder/renderer/src/paint.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::gpu_data::PaintData;
use crate::scene::Scene;
use crate::sorted_vector::SortedVector;
use indexmap::IndexSet;
use pathfinder_geometry::basic::line_segment::LineSegmentF32;
use pathfinder_geometry::basic::point::Point2DI32;
use pathfinder_geometry::basic::rect::RectI32;
use pathfinder_geometry::color::ColorU;

const PAINT_TEXTURE_WIDTH: i32 = 256;
const PAINT_TEXTURE_HEIGHT: i32 = 256;

const PAINT_TEXTURE_U_PER_TEXEL: i32 = 65536 / PAINT_TEXTURE_WIDTH;
const PAINT_TEXTURE_V_PER_TEXEL: i32 = 65536 / PAINT_TEXTURE_HEIGHT;

#[derive(Clone)]
pub(crate) struct Palette {
    pub(crate) paints: IndexSet<Paint>,
}

pub(crate) struct BuiltPalette {
    pub(crate) tex_coords: Vec<RectI32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Paint {
    Color(ColorU),
    LinearGradient(Box<LinearGradient>),
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PaintId(pub u16);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LinearGradient {
    pub stops: SortedVector<GradientStop>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Hash)]
pub struct GradientStop {
    // 16-bit normalized fixed point between [0, 1].
    pub distance: u16,
    // A monotonically increasing ID.
    pub id: u16,
    // The color.
    pub color: ColorU,
}

impl Palette {
    #[inline]
    pub(crate) fn new() -> Palette {
        Palette { paints: IndexSet::new() }
    }

    #[inline]
    pub(crate) fn push_paint(&mut self, paint: &Paint) -> PaintId {
        if let Some((paint_id, _)) = self.paints.get_full(paint) {
            return PaintId(paint_id as u16);
        }

        PaintId(self.paints.insert_full((*paint).clone()).0 as u16)
    }

    #[inline]
    pub(crate) fn get(&self, paint_id: PaintId) -> Option<&Paint> {
        self.paints.get_index(paint_id.0 as usize)
    }
}

impl Paint {
    pub(crate) fn is_opaque(&self) -> bool {
        match *self {
            Paint::Color(color) => color.is_opaque(),
            Paint::LinearGradient(ref gradient) => {
                gradient.stops.array.iter().all(|stop| stop.color.is_opaque())
            }
        }
    }
}

impl LinearGradient {
    #[inline]
    pub fn new() -> LinearGradient {
        LinearGradient { stops: SortedVector::new() }
    }

    #[inline]
    pub fn add_color_stop(&mut self, offset: f32, color: ColorU) {
        debug_assert!(offset >= 0.0 && offset <= 1.0);
        let distance = f32::round(offset * 65535.0) as u16;
        let id = self.stops.len() as u16;
        self.stops.push(GradientStop { distance, id, color });
    }
}

impl Palette {
    pub(crate) fn build(&self) -> BuiltPalette {
        let mut paint_tex_coords = vec![RectI32::default(); self.paints.len()];
        let mut next_tex_coord = Point2DI32::default();

        // Allocate linear gradients.
        let linear_gradient_alloc_size = Point2DI32::new(PAINT_TEXTURE_WIDTH, 1);
        for (paint_index, paint) in self.paints.iter().enumerate() {
            let gradient = match *paint {
                Paint::LinearGradient(ref gradient) => gradient,
                _ => continue,
            };
            paint_tex_coords[paint_index] = RectI32::new(next_tex_coord,
                                                         linear_gradient_alloc_size);
            next_tex_coord.set_y(next_tex_coord.y() + 1);
        }

        // Allocate colors.
        let color_alloc_size = Point2DI32::splat(1);
        for (paint_index, paint) in self.paints.iter().enumerate() {
            let color = match *paint { Paint::Color(color) => color, _ => continue };
            paint_tex_coords[paint_index] = RectI32::new(next_tex_coord, color_alloc_size);
            next_tex_coord.set_x(next_tex_coord.x() + 1);
            if next_tex_coord.x() >= PAINT_TEXTURE_WIDTH {
                next_tex_coord.set_x(0);
                next_tex_coord.set_y(next_tex_coord.y() + 1);
            }
        }

        BuiltPalette { tex_coords: paint_tex_coords }
    }
}

impl BuiltPalette {
    pub(crate) fn new() -> BuiltPalette {
        BuiltPalette { tex_coords: vec![] }
    }

    pub(crate) fn build_paint_data(&self, palette: &Palette) -> PaintData {
        let size = Point2DI32::new(PAINT_TEXTURE_WIDTH, PAINT_TEXTURE_HEIGHT);
        let mut paint_data = PaintData {
            size,
            texels: vec![0; size.x() as usize * size.y() as usize * 4],
        };
        for (paint_index, paint) in palette.paints.iter().enumerate() {
            let tex_coords = &self.tex_coords[paint_index];
            match *paint {
                Paint::Color(color) => paint_data.put_pixel(tex_coords.origin(), color),
                Paint::LinearGradient(ref gradient) => {
                    // FIXME(pcwalton)
                    let stop_count = gradient.stops.len();
                    for x in 0..PAINT_TEXTURE_WIDTH {
                        paint_data.put_pixel(tex_coords.origin() + Point2DI32::new(x, 0),
                                             gradient.stops.array[x as usize % stop_count].color);
                    }
                }
            }
        }
        paint_data
    }

    #[inline]
    pub(crate) fn tex_coords(&self, paint_id: PaintId) -> RectI32 {
        self.tex_coords[paint_id.0 as usize]
    }

    #[inline]
    pub(crate) fn norm_tex_coords(&self, paint_id: PaintId) -> Point2DI32 {
        let scale = Point2DI32::new(PAINT_TEXTURE_U_PER_TEXEL, PAINT_TEXTURE_V_PER_TEXEL);
        self.tex_coords(paint_id).origin().scale_xy(scale)
    }

    #[inline]
    pub(crate) fn half_texel() -> Point2DI32 {
        Point2DI32::new(PAINT_TEXTURE_U_PER_TEXEL / 2, PAINT_TEXTURE_V_PER_TEXEL / 2)
    }
}

impl PaintData {
    fn put_pixel(&mut self, coords: Point2DI32, color: ColorU) {
        // FIXME(pcwalton): I'm sure this is slow.
        let width = PAINT_TEXTURE_WIDTH as usize;
        let offset = (coords.y() as usize * width + coords.x() as usize) * 4;
        self.texels[offset + 0] = color.r;
        self.texels[offset + 1] = color.g;
        self.texels[offset + 2] = color.b;
        self.texels[offset + 3] = color.a;
    }
}
