param(
    [switch]
)

Write-Host "Running formatter" -ForegroundColor Cyan
cargo fmt

Write-Host "Running cargo check" -ForegroundColor Cyan
cargo check

if () {
    Write-Host "Running cargo test" -ForegroundColor Cyan
    cargo test
}
