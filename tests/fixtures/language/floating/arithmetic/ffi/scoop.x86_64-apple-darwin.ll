define { float, double } @scoop_pair(float %small, double %wide) nounwind {
    %first = insertvalue { float, double } zeroinitializer, float %small, 0
    %result = insertvalue { float, double } %first, double %wide, 1
    ret { float, double } %result
}
