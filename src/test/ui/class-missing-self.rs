struct cat {
  meows : usize,
}

impl cat {
    fn sleep(&self) { loop{} }
    fn meow(&self) {
      println!("Meow");
      meows += 1; //~ ERROR cannot find value `meows` in this scope
      sleep();     //~ ERROR cannot find function `sleep` in this scope
    }

}


 fn main() { }
