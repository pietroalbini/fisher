// Copyright (C) 2017 Pietro Albini
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

use std::collections::{HashMap, VecDeque};
use std::fmt::{self, Debug};
use std::sync::{Arc, Mutex, RwLock};
use std::sync::atomic::{AtomicUsize, Ordering};

use common::prelude::*;


pub struct Script<I: Send + Sync + Debug + Clone> {
    id: usize,
    name: String,
    can_be_parallel: bool,
    func: Arc<Mutex<Box<Fn(I) -> Result<()> + Send>>>,
}

impl<I: Send + Sync + Debug + Clone> ScriptTrait for Script<I> {
    type Id = usize;

    fn id(&self) -> usize {
        self.id
    }

    fn can_be_parallel(&self) -> bool {
        self.can_be_parallel
    }
}

impl<I: Send + Sync + Debug + Clone> Debug for Script<I> {

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Script {{ id: {}, name: {}, can_be_parallel: {} }}",
            self.id, self.name, self.can_be_parallel,
        )
    }
}


#[derive(Debug, Clone)]
pub struct Job<I: Send + Sync + Debug + Clone> {
    script: Arc<Script<I>>,
    args: I,
}

impl<I: Send + Sync + Debug + Clone> JobTrait<Script<I>> for Job<I> {
    type Context = ();
    type Output = ();

    fn execute(&self, _: &()) -> Result<()> {
        (self.script.func.lock().unwrap())(self.args.clone())
    }

    fn script_id(&self) -> usize {
        self.script.id
    }

    fn script_name(&self) -> &str {
        &self.script.name
    }
}


pub struct Repository<I: Send + Sync + Debug + Clone> {
    last_id: AtomicUsize,
    scripts: RwLock<HashMap<String, Arc<Script<I>>>>,
    ids: RwLock<Vec<usize>>,
}

impl<I: Send + Sync + Debug + Clone> Repository<I> {

    pub fn new() -> Self {
        Repository {
            last_id: AtomicUsize::new(0),
            ids: RwLock::new(Vec::new()),
            scripts: RwLock::new(HashMap::new()),
        }
    }

    pub fn add_script<F: Fn(I) -> Result<()> + 'static + Send>(
        &self, name: &str, parallel: bool, func: F
    ) {
        self.ids.write().unwrap().push(self.last_id.load(Ordering::SeqCst));
        self.scripts.write().unwrap().insert(name.to_string(), Arc::new(Script {
            id: self.last_id.fetch_add(1, Ordering::SeqCst),
            name: name.to_string(),
            can_be_parallel: parallel,
            func: Arc::new(Mutex::new(Box::new(func))),
        }));
    }

    pub fn job(&self, name: &str, args: I) -> Option<Job<I>> {
        self.scripts.read().unwrap().get(name).cloned()
                    .map(|script| Job { script, args })
    }

    pub fn hook_id_of(&self, name: &str) -> Option<usize> {
        self.scripts.read().unwrap().get(name).map(|script| script.id())
    }

    pub fn recreate_scripts(&self) {
        let mut scripts: Vec<_> = self.scripts.read().unwrap()
                                              .values().cloned().collect();

        self.ids.write().unwrap().clear();
        self.scripts.write().unwrap().clear();

        for script in scripts.drain(..) {
            self.add_script(
                &script.name,
                script.can_be_parallel,
                |_| { Ok(()) },
            );
        }
    }
}

impl<I: Send + Sync + Debug + Clone> ScriptsRepositoryTrait for Repository<I> {
    type Script = Script<I>;
    type Job = Job<I>;
    type ScriptsIter = SimpleIter<Arc<Script<I>>>;
    type JobsIter = SimpleIter<Job<I>>;

    fn id_exists(&self, id: &usize) -> bool {
        self.ids.read().unwrap().contains(id)
    }

    fn iter(&self) -> Self::ScriptsIter {
        SimpleIter::new(
            self.scripts.read().unwrap().values().cloned().collect()
        )
    }

    fn jobs_after_output(&self, _: ()) -> Option<Self::JobsIter> {
        None
    }
}


pub struct SimpleIter<T> {
    values: VecDeque<T>,
}

impl<T> SimpleIter<T> {

    fn new(values: VecDeque<T>) -> Self {
        SimpleIter { values }
    }
}

impl<T> Iterator for SimpleIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        self.values.pop_front()
    }
}


pub fn test_wrapper<F: Fn() -> Result<()>>(func: F) {
    let result = func();
    if let Err(error) = result {
        panic!("{}", error);
    }
}
