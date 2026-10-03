Output assets/demo.gif
Output assets/demo.mp4

Set Shell "zsh"
Set FontSize 14
Set Width 1280
Set Height 740
Set Padding 10
Set Theme "TokyoNight"
Set TypingSpeed 60ms
Set PlaybackSpeed 1.0

# Setup environment quietly inside clean generic demo repository
Hide
Type "export PATH=/Users/felipegodoi/Documents/repos/cli-diffviewer/target/release:$PATH"
Enter
Type "export PS1='nexus-api % '; cd /tmp/diffv-demo-repo; clear"
Enter
Sleep 600ms
Show

# 1. Launch diffv on working tree
Type "diffv --no-update"
Sleep 500ms
Enter
Sleep 600ms

# 2. Switch to Flat List view (t) immediately so src/auth.rs diff is displayed
Type "t"
Sleep 1.5s

# 3. Browse down through each modified file (diff updates in real time on the right)
Type "j"
Sleep 1.8s
Type "j"
Sleep 1.8s
Type "j"
Sleep 1.8s

# 4. Jump back up to src/auth.rs
Type "k"
Sleep 1.0s
Type "k"
Sleep 1.0s
Type "k"
Sleep 1.2s

# 5. Collapse sidebar (b) to maximize diff view across full terminal
Type "b"
Sleep 1.8s

# 6. Jump hunks inside full diff view
Type "]"
Sleep 1.5s
Type "["
Sleep 1.5s

# 7. Toggle between Side-by-Side and Unified mode
Type "m"
Sleep 2.0s
Type "m"
Sleep 1.8s

# 8. Re-open sidebar
Type "b"
Sleep 1.5s

# 9. Open Help modal
Type "?"
Sleep 2.5s
Escape 1
Sleep 1.0s

# 10. Quit diffv
Type "q"
Sleep 1s
