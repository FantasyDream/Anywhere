-- Return a count and NUL-delimited POSIX paths. Finder names may contain
-- commas, tabs, or newlines, but a POSIX path cannot contain NUL.
on run
    tell application id "com.apple.finder"
        set selectedItems to selection
        set selectedCount to count of selectedItems
        if selectedCount > 32 then error "too many selected Finder items" number 17201
        set serialized to selectedCount as text
        repeat with selectedItem in selectedItems
            set selectedPath to POSIX path of (selectedItem as alias)
            set serialized to serialized & (ASCII character 0) & selectedPath
        end repeat
        return serialized
    end tell
end run
