#[derive(Debug)]
pub struct ActivityLog {
    buffer: Vec<String>,
}

impl ActivityLog {
    pub fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    pub fn record(&mut self, event: String) {
        self.buffer.push(event);
        // if self.buffer.len() >= 64 {
        //     self.print_events();
        // }
    }

    pub fn print_events(&mut self) {
        if self.buffer.is_empty() {
            return;
        }

        let message = self.buffer.join("\n");
        println!("{message}");
        self.buffer.clear();
    }
}
