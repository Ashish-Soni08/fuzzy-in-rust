/*
 * dmoracle -- batch Double Metaphone oracle.
 *
 * Ground-truth harness for the fuzzy-in-rust port: links against the
 * ORIGINAL src/double_metaphone.c (read-only, Latin-1 bytes -- never
 * modify or re-save it) and exposes DoubleMetaphone as a stdin/stdout
 * batch filter.
 *
 * Protocol:
 *   stdin : one word per line (ASCII; an empty line means the empty string)
 *   stdout: exactly one line per input line, "<primary>|<secondary>"
 *
 * Output holds the RAW codes from the C library -- no wrapper semantics:
 * primary == secondary is printed twice (no None collapse), an empty code
 * prints as an empty string (so an empty input line prints "|"), and the
 * only truncation is the C's own 4-character cap.
 *
 * Build with build_oracle.ps1 (MSVC cl.exe; vcvars64 does not put cl.exe
 * on PATH on this machine, so the script sets PATH/INCLUDE/LIB manually --
 * see library/environment.md quirk #2).
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <io.h>
#include <fcntl.h>

#include "double_metaphone.h"

/*
 * Read one line from fp into a freshly malloc'd, NUL-terminated buffer.
 * Strips the trailing '\n' and a single trailing '\r' (CRLF tolerance).
 * Returns NULL at end of input (or on allocation failure). Caller frees.
 * The buffer grows dynamically, so arbitrarily long words are accepted.
 */
static char *read_line(FILE *fp)
{
    size_t cap = 128;
    size_t len = 0;
    char *buf;
    int c;

    buf = (char *)malloc(cap);
    if (buf == NULL)
	return NULL;

    while ((c = fgetc(fp)) != EOF && c != '\n')
      {
	  if (len + 1 >= cap)
	    {
		char *grown;

		cap *= 2;
		grown = (char *)realloc(buf, cap);
		if (grown == NULL)
		  {
		      free(buf);
		      return NULL;
		  }
		buf = grown;
	    }
	  buf[len++] = (char)c;
      }

    if (c == EOF && len == 0)
      {
	  free(buf);
	  return NULL;
      }

    if (len > 0 && buf[len - 1] == '\r')
	len--;

    buf[len] = '\0';
    return buf;
}

int
main(void)
{
    char *word;
    char *codes[2];
    int first_line = 1;

    /* Byte-exact streams: no CRLF translation on stdin/stdout. */
    _setmode(_fileno(stdin), _O_BINARY);
    _setmode(_fileno(stdout), _O_BINARY);

    while ((word = read_line(stdin)) != NULL)
      {
	  if (first_line)
	    {
		/*
		 * PowerShell 5.1 can prepend a UTF-8 BOM to the first
		 * piped line (mission library\environment.md quirk #10);
		 * strip it defensively, mirroring fuzzy-cli.
		 */
		first_line = 0;
		if (((unsigned char)word[0] == 0xEF)
		    && ((unsigned char)word[1] == 0xBB)
		    && ((unsigned char)word[2] == 0xBF))
		    memmove(word, word + 3, strlen(word + 3) + 1);
	    }

	  codes[0] = NULL;
	  codes[1] = NULL;

	  DoubleMetaphone(word, codes);

	  printf("%s|%s\n",
		 (codes[0] != NULL) ? codes[0] : "",
		 (codes[1] != NULL) ? codes[1] : "");

	  /*
	   * DoubleMetaphone hands ownership of both code buffers to the
	   * caller: primary and secondary are created with
	   * free_string_on_destroy == 0, so DestroyMetaString does not
	   * free them. free(NULL) is a no-op, so the NULL guards above
	   * make this safe even on a hypothetical allocation failure.
	   */
	  free(codes[0]);
	  free(codes[1]);
	  free(word);
      }

    return 0;
}
