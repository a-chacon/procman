//! This program is free software: you can redistribute it and/or modify
//! it under the terms of the GNU General Public License as published by
//! the Free Software Foundation, either version 3 of the License, or
//! (at your option) any later version.
//!
//! This program is distributed in the hope that it will be useful,
//! but WITHOUT ANY WARRANTY; without even the implied warranty of
//! MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//! GNU General Public License for more details.
//!
//! You should have received a copy of the GNU General Public License
//! along with this program.  If not, see <https://www.gnu.org/licenses/>.

use std::collections::HashMap;

/// Maps process names to their desired instance counts.
/// Supports an `all` wildcard that acts as the default for any unlisted process.
pub struct Formation(pub HashMap<String, usize>);

impl From<String> for Formation {
    fn from(s: String) -> Self {
        let map = s
            .split(',')
            .filter_map(|part| {
                let mut kv = part.splitn(2, '=');
                let key = kv.next()?.trim().to_string();
                let val: usize = kv.next()?.trim().parse().ok()?;
                Some((key, val))
            })
            .collect();
        Self(map)
    }
}

impl Formation {
    /// Returns the number of instances to run for a given process name.
    /// Falls back to the `all` entry if the name is not explicitly listed,
    /// and defaults to 1 if neither is present.
    pub fn count_for(&self, name: &str) -> usize {
        if let Some(&n) = self.0.get(name) {
            n
        } else if let Some(&n) = self.0.get("all") {
            n
        } else {
            1
        }
    }
}
