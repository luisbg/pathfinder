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
use pathfinder_geometry::color::ColorU;

const PAINT_TEXTURE_WIDTH: i32 = 256;
const PAINT_TEXTURE_HEIGHT: i32 = 256;

#[derive(Clone)]
pub(crate) struct Palette {
    pub(crate) paints: IndexSet<Paint>,
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
    pub(crate) fn build_paint_data(&self) -> PaintData {
        let size = Point2DI32::new(PAINT_TEXTURE_WIDTH, PAINT_TEXTURE_HEIGHT);
        let mut texels = vec![0; size.x() as usize * size.y() as usize * 4];
        for (paint_index, paint) in self.paints.iter().enumerate() {
            match *paint {
                Paint::Color(color) => {
                    texels[paint_index * 4 + 0] = color.r;
                    texels[paint_index * 4 + 1] = color.g;
                    texels[paint_index * 4 + 2] = color.b;
                    texels[paint_index * 4 + 3] = color.a;
                }
                Paint::LinearGradient(_) => {
                    // TODO(pcwalton)
                }
            }
        }
        PaintData { size, texels }
    }
}

pub(crate) fn paint_id_to_tex_coords(paint_id: PaintId) -> Point2DI32 {
    let tex_coords = Point2DI32::new(paint_id.0 as i32 % PAINT_TEXTURE_WIDTH,
                                     paint_id.0 as i32 / PAINT_TEXTURE_WIDTH);
    tex_coords.scale(256) + Point2DI32::new(128, 128)
}
