# CrashLogCollector
Script for Skyrim SE that grabs your latest crash logs and MO2 profile data, packaging them into clean text files ready for AI analysis.

What? What trickery is this?

I've recently discovered that you can use AI for diagnosis of your crashes. Comes especially handly when there's no crash log from CrashLogger or other plugin like that or you just don't feel like sending a cart of paperwork to a mod/modlist author so that he looks up your crash log and dismisses your problem outright.

The alternative is to gather whatever logs you have - which the game generates quite a few in the SKSE folder - and feed them to AI so it can find the solution for you and hold your hand like a baby, which is comforting when you're not a programmer. It works best when it can cross-reference them with your load order and modlist as a whole, so that's what we're going to fetch.

NotebookLLM however currently has a limit of 50 attachments, so it became impossible for me to do that without manual merging and changing the file extension to TXT, because Google allows only select file extensions to be uploaded. Doing this manually is tedious - thankfully, the AI made the script for me so I can share it with you.

This script automates the entire collection process. It scans the most recent logs and MO2 profile directories and compiles the necessary data into two parts, both clean, formatted text files: 1. MODLIST.txt file and LOGS file(s) divided into chunks of at most 450.000 word count as to be suitable for upload into NotebookLLM, Gemini, Claude or ChatGPT, which usually have limited context windows and permit a limited set of attachments to be uploaded. This allows you to easily upload your entire crash context to an AI in seconds, rather than manually copying and pasting dozens of individual files.

Features

    Automated Collection: Grabs the last 30 minutes of most recent crash logs from your SKSE folder.
    Profile Packaging: Compiles your key Mod Organizer 2 files (plugins.txt, modlist.txt, Skyrim.ini, SkyrimPrefs.ini).
    AI-Ready Format: Merges content into a few single text files with clear headers, perfect for uploading to LLM context windows.
    Non-Destructive: Your original logs and files remain untouched.

How to Use

    Download & Unpack: Extract the archive to any folder on your computer. Or run it as a mod from within MO2. Dealer's choice.
    Run the Tool: Double-click the file named Collector.bat.
    Select Logs Folder: A file explorer window will open. Navigate to and select your SKSE logs folder.
    Default location: Documents\My Games\Skyrim Special Edition\SKSE
    Select Profile Folder: A second window will open. Navigate to and select your active Mod Organizer 2 profile folder.
    Retrieve Files: Once the script finishes, new text files will appear in the script's folder, the number of which depends on your logs' size:
        LOGS_part_X.txt
        MO2 PROFILE.txt

"This tool includes a portable distribution of Python 3.11.9, which is licensed under the Python Software Foundation License."
