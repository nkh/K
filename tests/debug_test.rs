#[test]
fn debug_utf8_row_content() {
    use vrl_core::vtty::emulator::VttyEmulator;
    
    let mut emu = VttyEmulator::new(16, 80, 1000);
    
    // Feed just one string and check
    emu.feed_str("▽△◀▶◆●★✓✗");
    emu.feed_str("\n");
    
    let buf = emu.snapshot();
    
    eprintln!("Buffer width: {}, height: {}", buf.width, buf.height);
    eprintln!("Scrollback len: {}", buf.scrollback.len());
    
    for row_idx in 0..3 {
        let row = &buf.rows[row_idx];
        let chars: String = row.iter().take(20).map(|c| {
            if c.width == 0 { "·".to_string() } else { c.ch.to_string() }
        }).collect();
        let width_sum: usize = row.iter().map(|c| c.width as usize).sum();
        eprintln!("Row {}: width_sum={}, first 20: {:?}", row_idx, width_sum, chars);
    }
    
    // Now feed all strings
    let test_strings = &[
        "▽△◀▶◆●★✓✗",
        "┌──┐│└──┘├┤┬┴┼",
        "你好世界",
    ];
    let mut emu2 = VttyEmulator::new(16, 80, 1000);
    for s in test_strings {
        emu2.feed_str(s);
        emu2.feed_str("\n");
    }
    let buf2 = emu2.snapshot();
    eprintln!("\n=== After all 3 strings ===");
    for row_idx in 0..5 {
        let row = &buf2.rows[row_idx];
        let chars: String = row.iter().take(30).map(|c| {
            if c.width == 0 { "·".to_string() } else { c.ch.to_string() }
        }).collect();
        let width_sum: usize = row.iter().map(|c| c.width as usize).sum();
        eprintln!("Row {}: width_sum={}, first 30: {:?}", row_idx, width_sum, chars);
    }
    
    // Check HTML
    let html = vrl_core::vtty::renderer::VttyRenderer::to_html(&buf2);
    let lines: Vec<&str> = html.split('\n').take(5).collect();
    eprintln!("\nHTML first 5 lines:");
    for (i, line) in lines.iter().enumerate() {
        let text: String = line.chars().take(40).collect();
        eprintln!("  Line {}: {:?}", i, text);
    }
}
