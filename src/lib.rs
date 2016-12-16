#![feature(unsize)]
#![feature(coerce_unsized)] 
#![feature(shared)]
#![feature(heap_api)]
#![feature(alloc)]

use std::marker::Unsize;
use std::ops::CoerceUnsized;
use std::ptr::Shared;
use std::cell::RefCell;
use std::cell::RefMut;
use std::cell::Ref;
use std::cmp::PartialEq;
use std::fmt;
use std::ptr;
use std::marker::Sized;
use std::mem;

extern crate alloc;

use alloc::heap;

#[derive(Eq, PartialEq, Copy, Clone, Debug)]
enum GcColor {
        Unbound,
        Grey,
        White,
        Black
}


pub trait Mark: fmt::Debug {
    fn mark(&self, &mut InGcEnv)   ; 
}



impl< T: Mark+?Sized + Unsize<U>, U: Mark+?Sized> CoerceUnsized<Gc< U>> for Gc< T> {}

struct InGc<T: Mark+?Sized> {
    color: GcColor,
    valid: bool,
    size: usize,
    align: usize,
    content: RefCell<T>,
}


pub struct Gc<T: Mark+?Sized> {
    ptr: Shared<InGc<T>>
}

impl< T: Mark+?Sized> Copy for Gc< T> {}


impl< T: Mark+?Sized>  Gc< T> {
    fn mark(&self, gc: &mut InGcEnv)  { // where Mark: Sized {
        unsafe {
           (**self.ptr).content.borrow().mark(gc)
        }
    }
    fn color(&self) -> GcColor {
        unsafe {
            (**self.ptr).color
            //*(*self.ptr).color.borrow()
        }
    }

    fn set_color(&self, color: GcColor) {
        unsafe {
            //*(*self.ptr).color.borrow_mut() = color;
            (**self.ptr).color = color;
        }
    }
    fn forget(&self)  {
        unsafe {
            (**self.ptr).valid = false;
            //let p: *mut InGc<T> = self.ptr as *mut InGc<T>; 
            ptr::drop_in_place(*self.ptr);
            heap::deallocate((*self.ptr) as *mut u8,
                             (**self.ptr).size,
                             (**self.ptr).align);
        }
    }    

}



impl< T: 'static+Mark> Gc< T> {
     fn new(o: T, gc: &GcEnv) -> Gc<T>  {
        let white = if gc.inner.borrow().white_is_black { GcColor::Black} else { GcColor::White };
        unsafe {
        Gc {
            ptr: 
                Shared::new(
                Box::into_raw(
                Box::new(
                    InGc {
                         color: white,
                         valid: true,
                         size: mem::size_of::<T>(),
                         align: mem::align_of::<T>(),
                         content: RefCell::new(o), 
                    })))
        }
        }
    }
    pub fn mark_grey(&self, gc: &mut InGcEnv) {
        gc.mark_grey(*self);
    }
}

impl< T: Mark+?Sized> Gc< T> {
    pub fn borrow(&self) -> Ref<T> {
        unsafe {
            //println!("valid: {}",(*self.ptr).valid);
            assert!((**self.ptr).valid);
            (**self.ptr).content.borrow()
        }
    }    
    pub fn borrow_mut(& self) -> RefMut<T> {
        unsafe {
            //println!("valid: {}",(*self.ptr).valid);
            assert!((**self.ptr).valid);
            (**self.ptr).content.borrow_mut()
        }
    }    
    
}




impl< T: Mark+?Sized> Clone for Gc< T>  {
    fn clone(&self) -> Self {
        Gc { ptr:  self.ptr, }
    }
}




impl< T: Mark+?Sized> PartialEq for Gc< T>  {
    fn eq(&self, obj2: &Gc< T>) -> bool {
        //self.position() == obj2.position() && self.color() == self.color()
        *self.ptr == *obj2.ptr
    }
}

/*
impl<T: Mark+?Sized> Drop for Gc<T> {
    fn drop(&mut self) {
         println!("dropping gc");
    }
}
*/


impl< T: Mark+?Sized> fmt::Debug for Gc< T> {
    fn fmt(&self,  f: &mut fmt::Formatter) -> fmt::Result {
            //write!(f, "{:?} {:?}", self.color(), self.borrow())
            write!(f, "{:?} {:?}", self.color(), self.borrow())
    }
}


pub struct InGcEnv {
    whites: Vec<Gc< Mark>>,
    greys: Vec<Gc< Mark>>,
    blacks: Vec<Gc< Mark>>,
    roots: Vec<Gc< Mark>>,
    white_is_black: bool,
    
}

impl InGcEnv {

    fn col(&self, c: GcColor) -> &'static str {
        match c {
            GcColor::Unbound => { &"unbound" },
            GcColor::Grey => { &"grey" },
            GcColor::White => {
                if self.white_is_black {
                    &"black"
                } else {
                    &"white"
                }
            }
            GcColor::Black => {
                if self.white_is_black {
                    &"white"
                } else {
                    &"black"
                }
            }
        }
    }

   fn movec(&mut self, obj: Gc<Mark>, color: GcColor) {
        if obj.color() != color {
            if obj.color() != GcColor::Unbound {
                self.remove(&obj);
            }
            self.add(obj, color);
        } 
    } 
    fn remove(&mut self, obj: &Gc<Mark>) {
        let color = obj.color();
        obj.set_color(GcColor::Unbound);
        match color {
            GcColor::Unbound => { unreachable!(); },
            GcColor::Grey => {  
                //self.greys.remove(obj.position()); 
                let i = self.greys.iter().position(|e| { e==obj }).unwrap(); 
                self.greys.remove(i); 
            }
            GcColor::White => {
                if self.white_is_black {
                    let i = self.blacks.iter().position(|e| { e==obj }).unwrap(); 
                    self.blacks.remove(i);
                } else {
                    let i = self.whites.iter().position(|e| { e==obj }).unwrap(); 
                    self.whites.remove(i);
                }
            }
            GcColor::Black => {
                if self.white_is_black {
                    let i = self.whites.iter().position(|e| { e==obj }).unwrap(); 
                    self.whites.remove(i);
                } else {
                    let i = self.blacks.iter().position(|e| { e==obj }).unwrap(); 
                    self.blacks.remove(i);
                }
            }
        }
    }
    fn add(&mut self, obj: Gc<Mark>, color: GcColor) {
        obj.set_color(color);
        match color {
            GcColor::Unbound => { unreachable!(); },
            GcColor::Grey => {  
                self.greys.push(obj); 
            }
            GcColor::White => {
                if self.white_is_black {
                    self.blacks.push(obj);
                } else {
                    self.whites.push(obj);
                }
            }
            GcColor::Black => {
                if self.white_is_black {
                    self.whites.push(obj);
                } else {
                    self.blacks.push(obj);
                }
            }
        }
    }


    fn swap_white_and_black(&mut self) {
        
        println!("swap white and black");
         
        let tmp = self.whites.clone();
        self.whites = self.blacks.clone();
        self.blacks = tmp;
        
        self.white_is_black = !self.white_is_black;
        
    }

    fn mark_1(&mut self, obj: Gc<Mark>) {
        let black = if self.white_is_black { GcColor::White} else { GcColor::Black };
        //let white = if self.white_is_black { GcColor::Black} else { GcColor::White };
        self.movec(obj, black);
        obj.mark(self);
    }
    

    //pub fn mark_grey<T: 'static+Mark>(&mut self, obj: Gc<T>) {
    fn mark_grey(&mut self, obj: Gc<Mark>) {
         match obj.color() {
            GcColor::Unbound => { unreachable!(); },
            GcColor::Grey => {   }
            GcColor::White => {
                if self.white_is_black {
                } else {
                    self.movec(obj, GcColor::Grey);
                }
            }
            GcColor::Black => {
                if self.white_is_black {
                    self.movec(obj, GcColor::Grey);
                } else {
                }
            }
        }
    }
}

pub struct GcEnv {
    inner: RefCell<InGcEnv>
}


impl GcEnv {
    pub fn new() -> GcEnv {
        GcEnv {
            inner: RefCell::new(
                InGcEnv {
                    whites: vec![],
                    greys: vec![],
                    blacks:vec![] ,
                    roots: vec![],
                    white_is_black: false,
                })
        }
    }

    pub fn add_root(&self, obj: Gc<Mark>) {
        let mut gc = self.inner.borrow_mut();
        
        if !gc.roots.contains(&obj) {
            gc.roots.push(obj);
            gc.movec(obj, GcColor::Grey);
        }
        
    }

    pub fn new_gc<T: 'static+Mark>(&self, obj: T) -> Gc<T> {
        let gobj = Gc::<T>::new(obj, &self);
        let mut gc = self.inner.borrow_mut();
        gc.whites.push(gobj);
        return gobj;
    }

    //pub fn new_ref<T: 'static+Mark>(&self, o: Gc<T>, robj: Gc<T>) {
    pub fn new_ref(&self, o: Gc<Mark>, robj: Gc<Mark>) {
        let mut gc = self.inner.borrow_mut();
        if gc.white_is_black {
            if o.color() == GcColor::White && robj.color() == GcColor::Black {
                gc.movec(o, GcColor::Grey);
            }
        } else {
            if o.color() == GcColor::Black && robj.color() == GcColor::White {
                gc.movec(o, GcColor::Grey);
            }
        }
    }
    pub fn mark(&self, mut steps: u16) {
        println!("mark");
        let mut gc = self.inner.borrow_mut();
        while !gc.greys.is_empty() && steps > 0 {
            if let Some( obj) = gc.greys.pop() {
                obj.set_color(GcColor::Unbound);
                gc.mark_1(obj);
                steps = steps - 1;
            }
        }
        println!("end mark");
    }
    pub fn sweep(&self) {
        println!("sweep");
        let mut gc = self.inner.borrow_mut();
        println!("{:?}",gc.roots);
        println!("w {:?}",gc.whites);
        println!("b {:?}",gc.blacks);
        println!("g {:?}",gc.greys);
        while let Some(obj) = gc.greys.pop() {
            obj.set_color(GcColor::Unbound);
            gc.mark_1(obj);
        }

        while let Some(ref obj) = gc.whites.pop() {
            println!("forgetting {:?}", obj);
            obj.set_color(GcColor::Unbound);
            obj.forget();
        }
        gc.swap_white_and_black();
        let mut it = Vec::<Gc<Mark>>::new();
        for obj in gc.roots.iter() {
            println!("marking root {:?}", obj);
            it.push(*obj);
        }
        for obj in it  {
            gc.movec(obj, GcColor::Grey);
        }
    }
}


impl Drop for GcEnv {
    fn drop(&mut self) {
        println!("dropping GcEnv");
        
        let mut gc = self.inner.borrow_mut();

        gc.roots.clear();

        while let Some(ref obj) = gc.whites.pop() {
            obj.forget();
        }
        while let Some(ref obj) = gc.greys.pop() {
            obj.forget();
        }
        while let Some(ref obj) = gc.blacks.pop() {
            obj.forget();
        }
    }
}



/*
#[derive(Debug)]
enum ZCell {
    Atom,
    Compound(Gc<ZCell>, Gc<ZCell>),
}

impl Mark for ZCell {
    fn mark(&self, gc: &mut InGcEnv) {
        println!("marking {:?}", self);
        if let &ZCell::Compound(ref head, ref tail) = self {
            gc.mark_grey(head.clone());
            gc.mark_grey(tail.clone());
        }
    }
}

impl Drop for ZCell {
    fn drop(&mut self) {
        println!("dropping {:?}", self);
    }
}


#[macro_export]
macro_rules! gc {
    ($gc:expr, $expression:expr) => (
        $gc.new_gc($expression)
    )
}





fn main() {
   let gcenv = GcEnv::new();
   let zg2 = gc!(gcenv, ZCell::Atom);
   
   let zg3 = gc!(gcenv, ZCell::Atom);
   let zg1 = gc!(gcenv, ZCell::Compound(zg2.clone(), zg3.clone()));
   gcenv.add_root(zg1.clone());
   gcenv.new_ref(zg1.clone(), zg2.clone());
   gcenv.new_ref(zg1.clone(), zg3.clone());
   gcenv.mark(100);
   println!("sweep");
   gcenv.sweep();
   
}

*/