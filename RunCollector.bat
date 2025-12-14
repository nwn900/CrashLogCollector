@echo off
setlocal

:: --- Configuration ---
:: This looks for the folder "python_embed" right next to this script
set "PYTHON_EXE=python_embed\python.exe"
set "SCRIPT_FILE=collector.py"

:: --- Title ---
title Skyrim Crash Log Collector

:: --- Check if Python is correctly placed ---
if not exist "%PYTHON_EXE%" (
    echo [ERROR] Python not found!
    echo.
    echo Please make sure you have extracted the official Python Embeddable zip
    echo into a folder named 'python_embed' next to this script.
    echo.
    echo Expected path: %CD%\%PYTHON_EXE%
    echo.
    pause
    exit /b
)

:: --- Run the Python Script ---
:: -I: Isolated mode (ignores user environment variables for security/consistency)
:: -s: Don't add user site directory to sys.path
"%PYTHON_EXE%" -I -s "%SCRIPT_FILE%"

echo.
echo [INFO] Process finished.
pause