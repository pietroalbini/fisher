# Tutorial: send emails when scripts fails

This isn't a perfect world, and even the best scripts can fail: a way to know
when one of them doesn't execute correctly and why it failed is invaluable.

Fisher doesn't have a ready to be used solution for this though: there are
countless way you can be notified about a problem, and supporting all of them
would be an impossible task. Instead, Fisher provides you a simple but powerful
interface to script how notifications are sent: [status
hooks](../status-hooks.md).

A status hook is a normal scripts Fisher starts every time a job completed its
execution, that receives all the availabe information about the job execution.
This means it can then report those information to your monitoring software of
choice, or notify you about what happened the way you like.

In this tutorial we're going to create a status hook that sends an email with
the script output if an execution fails, so you're alerted when something goes
wrong and why.
