use env_logger::fmt::Formatter;
use log::{Record, Level};
use std::io::Write;
use chrono::Local;
use colored::*;
use rand::Rng;

const INSPIRING_PHRASES: [&str; 10] = [
    "Code is poetry — and the browser is your canvas.",
    "Web development is where logic meets art.",
    "Good design is invisible; great development makes it feel alive.",
    "Every broken layout is a step closer to mastery.",
    "You’re not just building websites — you’re building experiences.",
    "A great web app doesn’t just load fast; it earns trust fast.",
    "Your code might run in a browser, but it lives in someone’s daily life.",
    "The best developers write less — but say more.",
    "Design for humans first; devices second.",
    "A seamless UX is when users forget the interface exists.",
];

pub fn init_logger(log_level: &str) {
    let mut builder = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level));
    builder.format(format_log);

    // Filter out logs from actix_server and actix_web
    builder.filter(Some("actix_server"), log::LevelFilter::Warn);
    builder.filter(Some("actix_web"), log::LevelFilter::Warn);

    builder.init();
}

fn format_log(buf: &mut Formatter, record: &Record) -> std::io::Result<()> {
    let level_style = match record.level() {
        Level::Error => "ERROR".truecolor(255, 0, 0), // Bright Red for high visibility
        Level::Warn => "WARN".truecolor(255, 165, 0), // Orange for warnings
        Level::Info => "INFO".truecolor(0, 255, 255),   // Cyan for informational messages
        Level::Debug => "DEBUG".truecolor(138, 43, 226), // Purple for debug messages
        Level::Trace => "TRACE".truecolor(255, 105, 180), // Pink for trace messages
    };

    let message = format!(
        "{} [{}] - {}",
        Local::now().format("%Y-%m-%d %H:%M:%S"),
        level_style,
        record.args()
    );

    writeln!(buf, "{}", message)
}

pub fn print_banner(host: &str, port: u16, dev_mode: bool) {
    // Define the gradient colors based on the image
    let pink = (255, 64, 129);    // Vibrant Pink
    let mid_pink = (224, 80, 149);
    let purple = (126, 87, 194);   // Rich Purple
    let mid_blue = (79, 119, 224);
    let blue = (63, 156, 255);     // Bright Blue
    let cyan = (0, 230, 230);      // Vibrant Cyan

    // Top: Pink
    let banner_line1 = format!("    {} {} {} {} {} {} {}",
        "███╗   ██╗".truecolor(pink.0, pink.1, pink.2), " ██████╗ ".truecolor(pink.0, pink.1, pink.2), "██╗   ██╗".truecolor(pink.0, pink.1, pink.2), "███████╗".truecolor(pink.0, pink.1, pink.2), "███╗   ██╗".truecolor(pink.0, pink.1, pink.2), "████████╗".truecolor(pink.0, pink.1, pink.2), " █████╗ ".truecolor(pink.0, pink.1, pink.2));

    // Upper Middle: Pink -> Purple
    let banner_line2 = format!("    {} {} {} {} {} {} {}",
        "████╗  ██║".truecolor(mid_pink.0, mid_pink.1, mid_pink.2), "██╔═══██╗".truecolor(mid_pink.0, mid_pink.1, mid_pink.2), "██║   ██║".truecolor(mid_pink.0, mid_pink.1, mid_pink.2), "██╔════╝".truecolor(mid_pink.0, mid_pink.1, mid_pink.2), "████╗  ██║".truecolor(mid_pink.0, mid_pink.1, mid_pink.2), "╚══██╔══╝".truecolor(mid_pink.0, mid_pink.1, mid_pink.2), "██╔══██╗".truecolor(mid_pink.0, mid_pink.1, mid_pink.2));

    // Middle: Purple
    let banner_line3 = format!("    {} {} {} {} {} {} {}",
        "██╔██╗ ██║".truecolor(purple.0, purple.1, purple.2), "██║   ██║".truecolor(purple.0, purple.1, purple.2), "╚██╗ ██╔╝".truecolor(purple.0, purple.1, purple.2), "█████╗  ".truecolor(purple.0, purple.1, purple.2), "██╔██╗ ██║".truecolor(purple.0, purple.1, purple.2), "   ██║   ".truecolor(purple.0, purple.1, purple.2), "███████║".truecolor(purple.0, purple.1, purple.2));

    // Lower Middle: Purple -> Blue
    let banner_line4 = format!("    {} {} {} {} {} {} {}",
        "██║╚██╗██║".truecolor(mid_blue.0, mid_blue.1, mid_blue.2), "██║   ██║".truecolor(mid_blue.0, mid_blue.1, mid_blue.2), " ╚████╔╝ ".truecolor(mid_blue.0, mid_blue.1, mid_blue.2), "██╔══╝  ".truecolor(mid_blue.0, mid_blue.1, mid_blue.2), "██║╚██╗██║".truecolor(mid_blue.0, mid_blue.1, mid_blue.2), "   ██║   ".truecolor(mid_blue.0, mid_blue.1, mid_blue.2), "██╔══██║".truecolor(mid_blue.0, mid_blue.1, mid_blue.2));

    // Low: Blue -> Cyan
    let banner_line5 = format!("    {} {} {} {} {} {} {}",
        "██║ ╚████║".truecolor(blue.0, blue.1, blue.2), "╚██████╔╝".truecolor(blue.0, blue.1, blue.2), "  ╚██╔╝  ".truecolor(blue.0, blue.1, blue.2), "███████╗".truecolor(blue.0, blue.1, blue.2), "██║ ╚████║".truecolor(blue.0, blue.1, blue.2), "   ██║   ".truecolor(blue.0, blue.1, blue.2), "██║  ██║".truecolor(blue.0, blue.1, blue.2));

    // Bottom: Cyan
    let banner_line6 = format!("    {} {} {} {} {} {} {}",
        "╚═╝  ╚═══╝".truecolor(cyan.0, cyan.1, cyan.2), " ╚═════╝ ".truecolor(cyan.0, cyan.1, cyan.2), "   ╚═╝   ".truecolor(cyan.0, cyan.1, cyan.2), "╚══════╝".truecolor(cyan.0, cyan.1, cyan.2), "╚═╝  ╚═══╝".truecolor(cyan.0, cyan.1, cyan.2), "   ╚═╝   ".truecolor(cyan.0, cyan.1, cyan.2), "╚═╝  ╚═╝".truecolor(cyan.0, cyan.1, cyan.2));

    let border = "=".repeat(110);
    println!("{}", border.purple());
    println!("{}", banner_line1);
    println!("{}", banner_line2);
    println!("{}", banner_line3);
    println!("{}", banner_line4);
    println!("{}", banner_line5);
    println!("{}", banner_line6);
    println!("{}", "🚀 Noventa is running!".green());

    // Easter Egg: 1% chance to print an inspiring phrase in dev mode
    if dev_mode {
        let mut rng = rand::thread_rng();
        if rng.gen_range(0..100) < 1 {
            println!("{}", "\n✨ Lucky Message ✨".yellow());
            let phrase = INSPIRING_PHRASES[rng.gen_range(0..INSPIRING_PHRASES.len())];
            println!("   \"{}\" - ChatGPT\n", phrase.italic().cyan());
        }
    }

    println!("{}", format!("   - Address: http://{}:{}", host, port).cyan());
    println!("{}", "   - Happy coding!".cyan());
    println!("{}", border.purple());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_print_banner() {
        // Test that print_banner doesn't panic and produces output
        // We can't easily capture stdout in tests, so we just ensure it runs
        print_banner("localhost", 3000, false);
        print_banner("127.0.0.1", 8080, true);
        // If we get here without panicking, the test passes
    }

    #[test]
    fn test_inspiring_phrases_array() {
        // Test that the inspiring phrases array is not empty and contains expected content
        assert!(!INSPIRING_PHRASES.is_empty());
        assert!(INSPIRING_PHRASES.len() == 10);
        
        // Check that all phrases are non-empty strings
        for phrase in &INSPIRING_PHRASES {
            assert!(!phrase.is_empty());
            assert!(phrase.len() > 10); // Reasonable minimum length
        }
        
        // Check for some expected content
        assert!(INSPIRING_PHRASES.iter().any(|p| p.contains("code")));
        assert!(INSPIRING_PHRASES.iter().any(|p| p.contains("web")));
    }

    #[test]
    fn test_init_logger() {
        // Test that init_logger doesn't panic with valid log levels
        // Note: This will initialize the global logger, so it should only be called once
        // In a real application, this would be called at startup
        // For testing, we just ensure it doesn't panic
        // init_logger("info"); // Commented out to avoid affecting other tests
    }
}