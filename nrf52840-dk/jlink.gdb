# print demangled symbols by default
set print asm-demangle on

# Connect to the JLink GDB server
target remote :2331

# Enable SWO output
monitor SWO EnableTarget 0 0 1 0

# reset to start
monitor reset

# Load the program
load
