#![feature(const_generics, const_evaluatable_checked)]
#![allow(incomplete_features, unused)]

const fn complex_maths(n : usize) -> usize {
  2 * n + 1
}

pub struct Example<const N: usize> {
  a: [f32; N],
  b: [f32; complex_maths(N)],
  //~^ ERROR unconstrained generic
}

impl<const N: usize> Example<N> {
  pub fn new() -> Self {
    Self {
      a: [0.; N],
      b: [0.; complex_maths(N)],
    }
  }
}

impl Example<2> {
  pub fn sum(&self) -> f32 {
    self.a.iter().sum::<f32>() + self.b.iter().sum::<f32>()
  }
}

fn main() {}
