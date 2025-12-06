# 🌃 SAFE CODER - CYBERPUNK EDITION 🌃

## ⚡ NEON COLOR SCHEME ⚡

```
NEON CYAN     #00FFFF  ████████  Electric cyan - Primary actions, text
NEON MAGENTA  #FF00FF  ████████  Hot magenta - Secondary elements
NEON PURPLE   #A020F0  ████████  Deep purple - Borders, accents
NEON PINK     #FF1493  ████████  Hot pink - Highlights, warnings
NEON GREEN    #39FF14  ████████  Bright green - Success states
NEON BLUE     #7DF9FF  ████████  Electric blue - Dim text
CYBER RED     #FF0055  ████████  Cyberpunk red - Errors
PURE BLACK    #000000  ████████  Background
```

## 🎨 VISUAL EFFECTS

### 1. **PULSING NEON BORDERS**
- Borders alternate: CYAN ↔ MAGENTA every 1.5 seconds
- When focused: Faster pulse (1 second)
- When processing: INTENSE flash PINK ↔ MAGENTA (0.5 seconds)

### 2. **GLITCH EFFECTS**
- Random █ glitch appears every 5 seconds on project name
- Banner has RAPID_BLINK modifier for subtle flicker
- Cursor alternates CYAN ↔ MAGENTA continuously

### 3. **ANIMATED ELEMENTS**
- Status pulse: `▓` ↔ `▒` when session is ACTIVE
- Processing braille: 8-frame smooth cycle
- All caps text for MAXIMUM IMPACT

## 🎯 INTERFACE ELEMENTS

### **Banner (Neon Gradient)**
```
 ███████╗ █████╗ ███████╗███████╗  ← CYAN
 ██╔════╝██╔══██╗██╔════╝██╔════╝  ← MAGENTA
 ███████╗███████║█████╗  █████╗    ← CYAN
 ╚════██║██╔══██║██╔══╝  ██╔══╝    ← PINK
 ███████║██║  ██║██║     ███████╗  ← MAGENTA

▶ PROJECT: /your/path █  ← PURPLE text, glitch effect
```

### **Chat Panel: "▶▶ NEURAL LINK ◀◀"**
- Title in CYAN with PINK arrows
- Border pulses CYAN ↔ MAGENTA when focused
- Messages in full neon colors

### **System Status: "◢◤ SYSTEM STATUS ◢◤"**
- ◉ STATUS: ONLINE ▓  ← Green with pulse
- ▶ UPTIME: 5m 23s    ← Cyan
- ▶ MEMORY: 512 MB    ← Magenta
- ▶ vCPUs: 2          ← Purple

### **Tool Execution: "◢◤ TOOL EXECUTION ◢◤"**
- ▶ ◉ tool_name       ← Status icon changes color
  - PINK when running
  - GREEN when success
  - RED when failed

### **Input Area**
```
┌──────────────────────────────────┐ ← Pulsing CYAN/PURPLE
│  >> your input█                  │ ← PINK >>, CYAN cursor
│  ▶▶ ⣾⣿⣿⣿ PROCESSING... ◀◀        │ ← Flash PINK/MAGENTA
└──────────────────────────────────┘
```

## 🔄 DYNAMIC ANIMATIONS

### **Processing States (UPPERCASE + Flashing)**
1. `▶▶ ⣾⣿⣿⣿ ANALYZING REQUEST... ◀◀`
2. `▶▶ ⣽⣿⣿⣿ GENERATING RESPONSE.. ◀◀`
3. `▶▶ ⣻⣿⣿⣿ EXECUTING TOOLS. ◀◀`

**Flash Pattern**: PINK → MAGENTA → PINK (every 0.6 seconds)

### **Border Pulse Timings**
- **Idle**: 2-second cycle (CYAN ↔ PURPLE)
- **Focused**: 1-second cycle (CYAN ↔ MAGENTA)
- **Processing**: 0.5-second cycle (PINK ↔ MAGENTA)

### **Cursor Blink**
- Alternates: CYAN → MAGENTA every 1 second
- Uses RAPID_BLINK modifier for extra effect

### **Status Pulse**
- When ONLINE: `▓` → `▒` every 1 second
- Creates breathing/pulsing effect

## 🎮 MESSAGE COLORS

| Type      | Label Color | Icon | Text Color |
|-----------|-------------|------|------------|
| User      | CYAN        | 👤   | CYAN       |
| Assistant | GREEN       | 🤖   | GREEN      |
| System    | PINK        | ℹ️   | PINK       |
| Error     | RED         | ❌   | RED        |
| Tool      | MAGENTA     | 🔧   | MAGENTA    |

## 💥 CYBERPUNK SYMBOLS

```
▶▶ ◀◀  - Arrows (direction/emphasis)
◢◤     - Triangles (panel markers)
◉      - Circle (status indicators)
▓ ▒    - Blocks (pulse animation)
>>     - Input prompt
█      - Glitch/cursor
```

## 🚀 DEMO MODE

Run to experience the full effect:

```bash
./target/release/safe-coder chat --demo --path .
```

## 🌟 THE FULL EXPERIENCE

```
╔══════════════════════════════════════════════════════════════════╗
║  ███████╗ █████╗ ███████╗███████╗    ██████╗ ██████╗ ██████╗   ║ ← Alternating
║  ██╔════╝██╔══██╗██╔════╝██╔════╝    ██╔════╝██╔═══██╗██╔══██╗ ║   CYAN/MAGENTA
║  ███████╗███████║█████╗  █████╗      ██║     ██║   ██║██║  ██║ ║   with BLINK
║  ╚════██║██╔══██║██╔══╝  ██╔══╝      ██║     ██║   ██║██║  ██║ ║
║  ███████║██║  ██║██║     ███████╗    ╚██████╗╚██████╔╝██████╔╝ ║
║                                                                  ║
║         ▶ PROJECT: /safe-coder █                                ║ ← Glitch
╠══════════════════════════════════════╦═══════════════════════════╣
║ ▶▶ NEURAL LINK ◀◀                    ║ ◢◤ SYSTEM STATUS ◢◤       ║ ← Pulsing
║                                      ║ ◉ STATUS: ONLINE ▓        ║   borders
║ 👤 You: Make it cyberpunk           ║ ▶ UPTIME: 0m 42s          ║
║                                      ║ ▶ MEMORY: 512 MB          ║
║ 🤖 Assistant: ENGAGING NEON MODE    ║ ▶ vCPUs: 2                ║
║                                      ╠═══════════════════════════╣
║                                      ║ ◢◤ TOOL EXECUTION ◢◤      ║
║                                      ║ ▶ ◉ demo_tool             ║
╠══════════════════════════════════════╩═══════════════════════════╣
║  >> type here█                                                   ║ ← Blinking
║  ▶▶ ⣾⣿⣿⣿ PROCESSING... ◀◀                                        ║   PINK/MAG
╚══════════════════════════════════════════════════════════════════╝
```

## 🎪 WHAT MAKES IT CYBERPUNK

✅ **NEON COLORS** - Cyan, magenta, pink, purple everywhere
✅ **PULSING EFFECTS** - Everything breathes and glows
✅ **GLITCH VISUALS** - Random █ artifacts
✅ **ALL CAPS** - Processing messages scream
✅ **GEOMETRIC SHAPES** - ▶▶ ◀◀ ◢◤ ◉ everywhere
✅ **RAPID ANIMATIONS** - Fast flashing, constant movement
✅ **HIGH CONTRAST** - Pure black + bright neon
✅ **FUTURISTIC TEXT** - "NEURAL LINK", "SYSTEM STATUS"
✅ **INTENSE BORDERS** - Bold, pulsing, alternating
✅ **ELECTRIC VIBE** - Feels alive and charged

## 💎 COLOR THEORY

- **Cyan + Magenta** = Classic cyberpunk duo
- **Pure Black** = Maximum neon contrast
- **Pink accents** = Retro-futuristic flair
- **Green success** = Matrix vibes
- **Red errors** = Danger/warning aesthetic

This is NOT subtle. This is IN YOUR FACE. This is CYBERPUNK. 🔥⚡🌃
