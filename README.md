# inullify

This command-line utility watches given directories for new files, and for changes in existing files. If a dangerous file appears, it will be nullified (truncated to zero size). This is useful on FTP servers, and this can fight big percent of WordPress attacks.

A dangerous file is a file that matches given regular expression. By default `(?P<ELF>\x7FELF)|(?P<PHP><\?)`, that guards against uploading ELFs (linux executables) and PHP files.


## Usage

```bash
inullify

# or:
inullify /tmp /var/www/my-wordpress

# or:
inullify --regex='(?P<PHP><\?)' /tmp
```

To daemonize third-party software can be used. For example in Ubuntu we can use `daemon`:

```bash
sudo daemon --name=inullify --user=www-data --respawn --stdout=/tmp/inullify.log --stderr=/tmp/inullify-err.log -- inullify /tmp
```

iNullify must be run from user that has access to files of interest.


## Regex

Desired regex can be specified with `-r` or `--regex` command-line option.

You can mark alternatives with named groups. If a group matched for some file, this group name will be printed together with the filename.


## PHP antivirus

To prevent uploading dangerous files to server from PHP applications, you need to monitor PHP upload directory. This directory is set in `php.ini` with directive called `upload_tmp_dir`. By default PHP uses `/tmp`.
