pub fn main(){
    let maybe = Some(vec![true, true]);
    loop {
        if let Some(thing) = maybe {
//~^ ERROR use of partially moved value
//~| ERROR use of moved value
        }
    }
}
