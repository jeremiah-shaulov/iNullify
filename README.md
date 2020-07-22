# inullify

This command-line utility watches given directories for new files, and for changes in existing files. If a dangerous file appears, it will be nullified (truncated to zero size). This is useful on FTP servers, and this can fight big percent of WordPress attacks.

A dangerous file is a file that matches given regular expression. By default `(?P<ELF>\x7FELF)|(?P<PHP><\?)`, that guards against uploading ELFs (linux executables) and PHP files.

## Usage

```
inullify

# or:
inullify /tmp /var/www/my-wordpress

# or:
inullify --regex='(?P<PHP><\?)' /tmp
```

## Regex

You can mark alternatives with named groups. If a group matched for some file, this group name will be printed together with the filename.
