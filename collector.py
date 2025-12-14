import os
import sys
import subprocess
import datetime

# --- Configuration ---
MAX_WORDS_PER_CHUNK = 450000
LOG_FILENAME_TEMPLATE = "LOGS_part_{}.txt"
MO2_FILENAME = "MO2 PROFILE.txt"

def get_folder_path(prompt_text, initial_dir=""):
    """
    Opens a standard Windows Folder Browser Dialog.
    It uses a lightweight PowerShell command to access the dialog without 
    requiring extra Python libraries (like tkinter or pywin32) which are 
    not included in the standard embeddable package.
    """
    ps_command = f"""
    Add-Type -AssemblyName System.Windows.Forms
    $f = New-Object System.Windows.Forms.OpenFileDialog
    $f.Title = "{prompt_text}"
    $f.CheckFileExists = $false
    $f.CheckPathExists = $true
    $f.FileName = "Select Folder"
    $f.Filter = "Folders|`n"
    if ([System.IO.Directory]::Exists('{initial_dir}')) {{ $f.InitialDirectory = '{initial_dir}' }}
    $f.ShowDialog() | Out-Null
    return [System.IO.Path]::GetDirectoryName($f.FileName)
    """

    try:
        # We run PowerShell with -NoProfile to ensure speed and consistency
        cmd = ["powershell", "-NoProfile", "-Command", ps_command]
        
        # Creation flag to hide the console window popping up briefly (Windows only)
        startupinfo = subprocess.STARTUPINFO()
        startupinfo.dwFlags |= subprocess.STARTF_USESHOWWINDOW
        
        result = subprocess.check_output(cmd, text=True, startupinfo=startupinfo).strip()
        return result if result else None
    except Exception as e:
        print(f"[ERROR] Could not open folder dialog: {e}")
        return None

def count_words(text_line):
    """Simple whitespace-based word count."""
    return len(text_line.split())

def process_logs():
    print("--- Step 1: Skyrim Logs Collection ---")
    
    # Try to guess the default Documents location
    docs_path = os.path.expanduser("~\\Documents")
    default_log_path = os.path.join(docs_path, "My Games", "Skyrim Special Edition", "SKSE")
    
    print("Please select your SKSE Logs folder...")
    log_dir = get_folder_path("Navigate to SKSE Logs -> Click 'Open'", default_log_path)
    
    if not log_dir or not os.path.isdir(log_dir):
        print("[ERROR] No valid log directory selected. Skipping logs.")
        return

    print(f"Selected: {log_dir}")
    
    # Gather .log files
    log_files = []
    try:
        for entry in os.scandir(log_dir):
            if entry.is_file() and entry.name.lower().endswith(".log"):
                log_files.append(entry)
    except OSError as e:
        print(f"[ERROR] Accessing directory: {e}")
        return

    if not log_files:
        print("[WARNING] No .log files found in directory.")
        return

    # Sort by modification time (Newest first)
    log_files.sort(key=lambda f: f.stat().st_mtime, reverse=True)
    
    # Filter by 30 minute window relative to the NEWEST file found
    if not log_files:
        return
        
    newest_time = log_files[0].stat().st_mtime
    cutoff_time = newest_time - (30 * 60) # 30 minutes in seconds
    
    recent_logs = [f for f in log_files if f.stat().st_mtime >= cutoff_time]
    
    print(f"Found {len(recent_logs)} logs in the last 30 minute session.")
    
    # --- Chunking Logic ---
    current_chunk_index = 1
    current_word_count = 0
    
    # Open the first chunk file
    current_out_file = open(LOG_FILENAME_TEMPLATE.format(current_chunk_index), "w", encoding="utf-8")
    
    try:
        for log_entry in recent_logs:
            # Create a header for this specific log file
            log_timestamp = datetime.datetime.fromtimestamp(log_entry.stat().st_mtime)
            header = f"\n--- Log File: {log_entry.name} [{log_timestamp}] ---\n"
            header_words = count_words(header)
            
            # If header itself won't fit (unlikely), force new chunk
            if current_word_count + header_words > MAX_WORDS_PER_CHUNK:
                current_out_file.close()
                current_chunk_index += 1
                current_out_file = open(LOG_FILENAME_TEMPLATE.format(current_chunk_index), "w", encoding="utf-8")
                current_word_count = 0
            
            current_out_file.write(header)
            current_word_count += header_words
            
            # Read file content
            try:
                with open(log_entry.path, "r", encoding="utf-8", errors="replace") as infile:
                    for line in infile:
                        words_in_line = count_words(line)
                        
                        # CRITICAL CHECK:
                        # If adding this line exceeds the limit, close current file and start a new one.
                        # This ensures we never split *inside* a line.
                        if current_word_count + words_in_line > MAX_WORDS_PER_CHUNK:
                            current_out_file.close()
                            print(f"[INFO] Chunk {current_chunk_index} filled. Starting chunk {current_chunk_index + 1}...")
                            
                            current_chunk_index += 1
                            current_out_file = open(LOG_FILENAME_TEMPLATE.format(current_chunk_index), "w", encoding="utf-8")
                            current_word_count = 0
                        
                        current_out_file.write(line)
                        current_word_count += words_in_line
                        
            except Exception as e:
                print(f"[ERROR] Could not read {log_entry.name}: {e}")
                
    finally:
        current_out_file.close()
        print(f"[INFO] Logs processing complete. Total chunks generated: {current_chunk_index}")

def process_mo2():
    print("\n--- Step 2: MO2 Profile Collection ---")
    print("Please select your MO2 Profile folder...")
    
    mo2_dir = get_folder_path("Navigate to MO2 Profile -> Click 'Open'")
    
    if not mo2_dir or not os.path.isdir(mo2_dir):
        print("[ERROR] No valid MO2 directory selected. Skipping MO2 files.")
        return

    print(f"Selected: {mo2_dir}")

    # Get .txt and .ini files
    files_to_process = []
    try:
        for entry in os.scandir(mo2_dir):
            if entry.is_file() and entry.name.lower().endswith(('.txt', '.ini')):
                files_to_process.append(entry)
    except OSError:
        pass
            
    try:
        with open(MO2_FILENAME, "w", encoding="utf-8") as outfile:
            if not files_to_process:
                outfile.write("No text or ini files found in selected folder.")
                print("[WARNING] No .txt or .ini files found in profile.")
            else:
                for f_entry in files_to_process:
                    outfile.write(f"--- Profile File: {f_entry.name} ---\n")
                    try:
                        with open(f_entry.path, "r", encoding="utf-8", errors="replace") as infile:
                            outfile.write(infile.read())
                        outfile.write("\n\n")
                    except Exception as e:
                        outfile.write(f"[Error reading file: {e}]\n")
        print(f"[INFO] MO2 Profile data saved to '{MO2_FILENAME}'")
    except Exception as e:
        print(f"[ERROR] Failed to write MO2 output file: {e}")

def main():
    try:
        process_logs()
        process_mo2()
        print("\n[SUCCESS] Operation complete.")
    except Exception as e:
        print(f"\n[CRITICAL ERROR] {e}")

if __name__ == "__main__":
    main()