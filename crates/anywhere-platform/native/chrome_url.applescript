-- Read only the active tab of Chrome's front window. Do not activate Chrome,
-- inspect page content, or guess a URL when no browser window exists.
if application id "com.google.Chrome" is not running then return ""
tell application id "com.google.Chrome"
    if (count of windows) is 0 then return ""
    return URL of active tab of front window
end tell
