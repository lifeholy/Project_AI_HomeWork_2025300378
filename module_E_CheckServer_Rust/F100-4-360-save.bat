@echo off
setlocal EnableExtensions EnableDelayedExpansion

:: =====================================================================
:: Studio Airbook - FAS and EAI Server Backup Automation
:: Inspection Code : F100-4-360-save (Annual Folder Backup)
:: Target OS       : Windows Server 2025 / Closed Network
:: Engine          : Built-in tar.exe (Zip Compression)
:: Note            : Locale-Safe Timestamp / 100% Pure ASCII
:: =====================================================================

:: 1. Timestamp Generation (Universal PowerShell Format)
for /f "usebackq tokens=1,2" %%A in (`powershell -NoProfile -Command "Get-Date -Format 'yyyyMMdd HHmmss'"`) do (
    set "DATE_PART=%%A"
    set "TIME_PART=%%B"
)
set "TIMESTAMP=%DATE_PART%_%TIME_PART%"

:: 2. Base and Destination Directory Setup
set "BASE_DIR=C:\checkPMI"
set "SAVE_DIR=%BASE_DIR%\save360"
set "LOG_FILE=%SAVE_DIR%\backup_history.log"

if not exist "%BASE_DIR%" md "%BASE_DIR%"
if not exist "%SAVE_DIR%" md "%SAVE_DIR%"

echo ==============================================================================
echo [F100-4-360-save] Annual Folder Backup Process Start...
echo Timestamp   : %TIMESTAMP%
echo Destination : %SAVE_DIR%
echo ==============================================================================
echo.

echo [%TIMESTAMP%] --- Backup Process Started --- >> "%LOG_FILE%"

:: 3. Target Folder Configuration
:: Format: call :COMPRESS_FOLDER "Folder_Path"

:: --- Active Target Folders ---
call :COMPRESS_FOLDER "C:\checkPMI\logs"

:: --- Commented Target Folders (Uncomment REM when needed) ---
REM call :COMPRESS_FOLDER "C:\Siemens\DesigoCC"
REM call :COMPRESS_FOLDER "C:\Program Files\Microsoft SQL Server\MSSQL16.MSSQLSERVER\MSSQL\Log"
REM call :COMPRESS_FOLDER "C:\EAI_Interface\Data"
REM call :COMPRESS_FOLDER "C:\FAS_Data"

echo.
echo ==============================================================================
echo All designated folders have been processed.
echo Check compressed archives in: %SAVE_DIR%
echo ==============================================================================
echo.

echo [%TIMESTAMP%] --- Backup Process Finished --- >> "%LOG_FILE%"
echo. >> "%LOG_FILE%"

exit /b 0

:: =====================================================================
:: Folder Compression Subroutine (Using Built-in tar.exe)
:: =====================================================================
:COMPRESS_FOLDER
set "SRC_DIR=%~1"

if not exist "%SRC_DIR%" (
    echo [SKIP] Directory not found: "%SRC_DIR%"
    echo [%TIMESTAMP%] [SKIP] Directory not found: "%SRC_DIR%" >> "%LOG_FILE%"
    exit /b 0
)

for %%F in ("%SRC_DIR%") do set "FOLDER_NAME=%%~nxF"
if "%FOLDER_NAME%"=="" set "FOLDER_NAME=RootDrive"

set "ARCHIVE_FILE=%SAVE_DIR%\%FOLDER_NAME%_%TIMESTAMP%.zip"

echo [*] Compressing: "%SRC_DIR%"
echo     Archive -^> "%ARCHIVE_FILE%"

tar -a -cf "%ARCHIVE_FILE%" -C "%SRC_DIR%\.." "%FOLDER_NAME%" >nul 2>&1

if %errorlevel% equ 0 (
    echo [PASS] Backup Successful: "%FOLDER_NAME%_%TIMESTAMP%.zip"
    echo [%TIMESTAMP%] [PASS] Backup Success: "%SRC_DIR%" -^> "%ARCHIVE_FILE%" >> "%LOG_FILE%"
) else (
    echo [FAIL] Error occurred while compressing: "%SRC_DIR%"
    echo [%TIMESTAMP%] [FAIL] Backup Failed: "%SRC_DIR%" >> "%LOG_FILE%"
)

exit /b 0