tell application "Finder"
    if (count of windows) is 0 then
        return POSIX path of (path to desktop folder)
    end if
    try
        return POSIX path of (target of front window as alias)
    on error messageText number errorNumber
        -- Recents, Shared and other virtual locations have no filesystem alias.
        -- Permission/transport failures must still reach the caller.
        if errorNumber is -1700 or errorNumber is -1728 then return ""
        error messageText number errorNumber
    end try
end tell
