alias wd := watch-doc

# npm install -g browser-sync
watch-doc:
    browser-sync start -w --ss target/doc -s target/doc --directory &
    cargo watch -s 'cargo doc'
