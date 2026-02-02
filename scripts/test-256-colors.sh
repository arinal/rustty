#!/bin/bash

# Test script to display the full 256-color ANSI palette
# Run this inside the Rustty terminal to verify color support

echo "=== 256-Color ANSI Palette Test ==="
echo ""

echo "Standard Colors (0-15):"
for i in {0..15}; do
    printf "\e[48;5;${i}m  %3d  \e[0m" "$i"
    if [ $((($i + 1) % 8)) -eq 0 ]; then
        echo ""
    fi
done
echo ""

echo ""
echo "6x6x6 RGB Cube (16-231):"
for r in {0..5}; do
    for g in {0..5}; do
        for b in {0..5}; do
            index=$((16 + r*36 + g*6 + b))
            printf "\e[48;5;${index}m %3d \e[0m" "$index"
        done
        echo ""
    done
    echo ""
done

echo ""
echo "Grayscale Ramp (232-255):"
for i in {232..255}; do
    printf "\e[48;5;${i}m %3d \e[0m" "$i"
    if [ $((($i - 232 + 1) % 12)) -eq 0 ]; then
        echo ""
    fi
done
echo ""

echo ""
echo "=== Color Test Complete ==="
echo "If you see colored blocks with numbers, 256-color support is working!"
