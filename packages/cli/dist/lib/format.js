/** ANSI color helpers for terminal output. */
const GREEN = "\x1b[32m";
const RED = "\x1b[31m";
const YELLOW = "\x1b[33m";
const GRAY = "\x1b[90m";
const BOLD = "\x1b[1m";
const DIM = "\x1b[2m";
const RESET = "\x1b[0m";
export function green(s) {
    return `${GREEN}${s}${RESET}`;
}
export function red(s) {
    return `${RED}${s}${RESET}`;
}
export function yellow(s) {
    return `${YELLOW}${s}${RESET}`;
}
export function gray(s) {
    return `${GRAY}${s}${RESET}`;
}
export function bold(s) {
    return `${BOLD}${s}${RESET}`;
}
export function dim(s) {
    return `${DIM}${s}${RESET}`;
}
export function ok(label) {
    return `${GREEN}OK${RESET}     ${label}`;
}
export function fail(label) {
    return `${RED}FAIL${RESET}   ${label}`;
}
export function check(label) {
    return `  ${GREEN}\u2713${RESET} ${label}`;
}
export function cross(label) {
    return `  ${RED}\u2717${RESET} ${label}`;
}
export function circle(label) {
    return `  ${GRAY}\u25CB${RESET} ${label}`;
}
export function padRight(s, len) {
    return s + " ".repeat(Math.max(0, len - s.length));
}
export function heading(title) {
    console.log(`\n${BOLD}${title}${RESET}`);
    console.log("=".repeat(title.length));
}
export function subheading(title) {
    console.log(`\n${BOLD}${title}${RESET}`);
}
//# sourceMappingURL=format.js.map