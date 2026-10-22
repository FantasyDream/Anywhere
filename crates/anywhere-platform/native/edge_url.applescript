-- Read only the active tab of Edge's front window. Do not activate Edge,
-- inspect page content, or guess a URL when no browser window exists.
if application id "com.microsoft.edgemac" is not running then return ""
tell application id "com.microsoft.edgemac"
    if (count of windows) is 0 then return ""
    return URL of active tab of front window
end tell
