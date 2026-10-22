-- Read only the selected tab of Safari's front window. Do not activate Safari,
-- inspect page content, or guess a URL when no browser window exists.
if application id "com.apple.Safari" is not running then return ""
tell application id "com.apple.Safari"
    if (count of windows) is 0 then return ""
    return URL of current tab of front window
end tell
