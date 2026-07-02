## Windows / pwsh port TODO

### Core (make it work well on Windows)
- [x] Prefer pwsh 7+ over cmd.exe for execution (env.rs, with cmd fallback)
- [ ] Adjust command exec args for pwsh if needed (verify after shell swap)
- [ ] Make command generation produce pwsh-style commands on Windows (command_generator prompt)

### Slim down (self-use build)
- [ ] Remove unused providers & telemetry (AWS/GCP/posthog/tracker)

### TUI

- [ ] Add TUI autocompletion
- [ ] Better rendering includes img, MD highlight, Vim keybind support, etc.

### Distribution
- [ ] npm wrapper package for `npm i -g` on Windows
