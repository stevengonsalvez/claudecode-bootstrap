#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.8"
# dependencies = [
#     "pyttsx3>=2.90",
# ]
# ///

# ABOUTME: Cross-platform TTS script that automatically selects the best TTS method based on OS
# Uses macOS 'say' command, Windows SAPI, or falls back to pyttsx3 for maximum compatibility

import sys
import subprocess
import platform
import random
import shutil

def get_platform_tts_method():
    """Detect the best TTS method for the current platform."""
    system = platform.system().lower()
    
    if system == "darwin":  # macOS
        # Check if 'say' command is available
        if shutil.which('say'):
            return "macos_say"
    elif system == "windows":
        # Windows has built-in SAPI
        return "windows_sapi" 
    elif system == "linux":
        # Check for common Linux TTS commands
        if shutil.which('espeak'):
            return "linux_espeak"
        elif shutil.which('spd-say'):
            return "linux_spd_say"
        elif shutil.which('festival'):
            return "linux_festival"
    
    # Fall back to pyttsx3 for all platforms
    return "pyttsx3"

def speak_macos(text):
    """Use macOS native 'say' command."""
    try:
        result = subprocess.run(['say', text], 
                              capture_output=True, 
                              text=True, 
                              timeout=30)
        return result.returncode == 0
    except Exception:
        return False

def speak_windows_sapi(text):
    """Use Windows SAPI via PowerShell."""
    try:
        # Use PowerShell to access Windows Speech API
        ps_command = f'Add-Type -AssemblyName System.Speech; (New-Object System.Speech.Synthesis.SpeechSynthesizer).Speak("{text}")'
        result = subprocess.run(['powershell', '-Command', ps_command],
                              capture_output=True,
                              text=True,
                              timeout=30)
        return result.returncode == 0
    except Exception:
        return False

def speak_linux_espeak(text):
    """Use Linux espeak command."""
    try:
        result = subprocess.run(['espeak', text],
                              capture_output=True,
                              text=True,
                              timeout=30)
        return result.returncode == 0
    except Exception:
        return False

def speak_linux_spd_say(text):
    """Use Linux spd-say command."""
    try:
        result = subprocess.run(['spd-say', text],
                              capture_output=True,
                              text=True,
                              timeout=30)
        return result.returncode == 0
    except Exception:
        return False

def speak_linux_festival(text):
    """Use Linux festival command."""
    try:
        # Festival expects text via stdin
        result = subprocess.run(['festival', '--tts'],
                              input=text,
                              text=True,
                              capture_output=True,
                              timeout=30)
        return result.returncode == 0
    except Exception:
        return False

def speak_pyttsx3(text):
    """Use pyttsx3 as fallback TTS."""
    try:
        import pyttsx3
        engine = pyttsx3.init()
        engine.say(text)
        engine.runAndWait()
        return True
    except Exception:
        return False

def speak_text(text, method):
    """Speak text using the specified method."""
    method_map = {
        "macos_say": speak_macos,
        "windows_sapi": speak_windows_sapi,
        "linux_espeak": speak_linux_espeak,
        "linux_spd_say": speak_linux_spd_say,
        "linux_festival": speak_linux_festival,
        "pyttsx3": speak_pyttsx3
    }
    
    if method in method_map:
        return method_map[method](text)
    else:
        return False

def main():
    try:
        # Detect platform and TTS method
        tts_method = get_platform_tts_method()
        platform_name = platform.system()
        
        print(f"🎙️  Cross-Platform TTS ({platform_name})")
        print("=" * 35)
        print(f"🔧 Method: {tts_method}")
        
        # Get text from command line argument or use default
        if len(sys.argv) > 1:
            text = " ".join(sys.argv[1:])  # Join all arguments as text
        else:
            # Default completion messages
            completion_messages = [
                "Work complete!",
                "All done!",
                "Task finished!",
                "Job complete!",
                "Ready for next task!",
                "Claude Code task completed!"
            ]
            text = random.choice(completion_messages)
        
        print(f"🎯 Text: {text}")
        print("🔊 Speaking...")
        
        # Attempt to speak using the selected method
        success = speak_text(text, tts_method)
        
        if success:
            print("✅ Speech completed!")
        else:
            print(f"❌ {tts_method} failed, trying pyttsx3 fallback...")
            # Try pyttsx3 as last resort
            if tts_method != "pyttsx3":
                fallback_success = speak_pyttsx3(text)
                if fallback_success:
                    print("✅ Fallback speech completed!")
                else:
                    print("❌ All TTS methods failed!")
                    
    except Exception as e:
        print(f"❌ Error: {e}")

if __name__ == "__main__":
    main()
